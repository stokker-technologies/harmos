/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
// Local Imports
use crate::{
    Application, state::Entry, state::Position, state::PreparationError, state::Receipt,
    state::Recorded, state::RequestKey,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct CaptureLimits {
    pub pending_bytes: u32,
    pub max_group_bytes: u32,
}

pub(crate) struct CallbackDeclaration<A: Application> {
    pub limits: CaptureLimits,
    pub consumer: Box<dyn Capturing<A>>,
}

struct Group<A: Application> {
    entries: Vec<Arc<Entry<Recorded<A>>>>,
    frontier: Position,
    _permit: OwnedSemaphorePermit,
}

pub(crate) struct Callback<A: Application> {
    sender: mpsc::UnboundedSender<Group<A>>,
    shared: Arc<Shared>,
    limits: CaptureLimits,
    worker: tokio::task::JoinHandle<()>,
}

struct Shared {
    failed: Arc<AtomicBool>,
    signal: watch::Sender<bool>,
    capacity: Arc<Semaphore>,
    progress: watch::Sender<Result<Position, CaptureError>>,
}

struct Failure {
    shared: Arc<Shared>,
    clean: bool,
}

/* === Enums === */
pub(crate) enum Declaration<A: Application> {
    Callback(CallbackDeclaration<A>),
    Managed {
        storage: crate::storage::StorageManager<crate::storage::ErasedCatalog>,
        receipts: HashMap<RequestKey, Receipt>,
        projection: Option<crate::storage::capture::Projection>,
        snapshots: Option<crate::state::snapshot::Snapshots<A>>,
    },
}

pub(crate) enum Capture<A: Application> {
    Callback(Callback<A>),
    Managed {
        storage: crate::storage::StorageManager<crate::storage::ErasedCatalog>,
        receipts: HashMap<RequestKey, Receipt>,
        projection: Option<crate::storage::capture::Projection>,
        snapshots: Option<crate::state::snapshot::Snapshots<A>>,
        failed: Arc<AtomicBool>,
        progress: watch::Sender<Result<Position, CaptureError>>,
        signal: watch::Sender<bool>,
    },
}

pub(crate) enum Permit {
    Callback(OwnedSemaphorePermit),
    Managed(crate::storage::state::Permit),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("required journal capture failed")]
    Failed,
    #[error("capture limits must be positive and group bytes must fit pending bytes")]
    InvalidLimits,
}

/* === Traits === */
pub trait JournalCapture<A: Application>: Send + 'static {
    fn capture(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        frontier: Position,
    ) -> impl Future<Output = Result<(), CaptureError>> + Send;
}

pub(crate) trait Capturing<A: Application>: Send {
    fn capture(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        frontier: Position,
    ) -> Pin<Box<dyn Future<Output = Result<(), CaptureError>> + Send + '_>>;
}

/* === Implementations === */
impl<A: Application, C: JournalCapture<A>> Capturing<A> for C {
    fn capture(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        frontier: Position,
    ) -> Pin<Box<dyn Future<Output = Result<(), CaptureError>> + Send + '_>> {
        Box::pin(JournalCapture::capture(self, entries, frontier))
    }
}

impl Shared {
    fn fail(&self) {
        self.failed.store(true, Ordering::Release);
        self.capacity.close();
        let _ = self.progress.send_replace(Err(CaptureError::Failed));
        self.signal.send_replace(true);
    }
}

impl Drop for Failure {
    fn drop(&mut self) {
        if !self.clean {
            self.shared.fail();
        }
    }
}

impl<A: Application> CallbackDeclaration<A> {
    pub fn start(self, failed: Arc<AtomicBool>, origin: Position) -> Callback<A> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Group<A>>();
        let (signal, _) = watch::channel(false);
        let (progress, _) = watch::channel(Ok(origin));
        let shared = Arc::new(Shared {
            failed,
            signal,
            progress,
            capacity: Arc::new(Semaphore::new(self.limits.pending_bytes as usize)),
        });
        let mut guard = Failure {
            shared: shared.clone(),
            clean: false,
        };
        let mut consumer = self.consumer;
        let worker = tokio::spawn(async move {
            while let Some(group) = receiver.recv().await {
                if consumer
                    .capture(group.entries, group.frontier)
                    .await
                    .is_err()
                {
                    return;
                }
                // Capacity includes the callback until successful acknowledgement.
                let _ = guard.shared.progress.send_replace(Ok(group.frontier));
                drop(group._permit);
            }
            guard.clean = true;
            drop(guard);
        });
        Callback {
            sender,
            shared,
            limits: self.limits,
            worker,
        }
    }
}

impl<A: Application> Callback<A> {
    pub async fn reserve(&self, bytes: usize) -> Result<OwnedSemaphorePermit, PreparationError> {
        self.check()?;
        if bytes > self.limits.max_group_bytes as usize {
            return Err(PreparationError::Limit);
        }
        self.shared
            .capacity
            .clone()
            .acquire_many_owned(bytes as u32)
            .await
            .map_err(|_| PreparationError::CaptureFailed)
    }

    pub fn check(&self) -> Result<(), PreparationError> {
        if self.shared.failed.load(Ordering::Acquire) {
            Err(PreparationError::CaptureFailed)
        } else {
            Ok(())
        }
    }

    pub fn submit(
        &self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        frontier: Position,
        permit: OwnedSemaphorePermit,
    ) {
        if self
            .sender
            .send(Group {
                entries,
                frontier,
                _permit: permit,
            })
            .is_err()
        {
            self.shared.fail();
        }
    }

    pub fn progress(&self) -> watch::Receiver<Result<Position, CaptureError>> {
        self.shared.progress.subscribe()
    }

    pub fn failure(&self) -> watch::Receiver<bool> {
        self.shared.signal.subscribe()
    }

    pub async fn finish(self) {
        drop(self.sender);
        let _ = self.worker.await;
    }
}

impl<A: Application> Declaration<A> {
    pub(crate) fn start(self, failed: Arc<AtomicBool>, origin: Position) -> Capture<A> {
        match self {
            Self::Callback(callback) => Capture::Callback(callback.start(failed, origin)),
            Self::Managed {
                storage,
                receipts,
                projection,
                snapshots,
            } => Capture::Managed {
                storage,
                receipts,
                projection,
                snapshots,
                failed,
                progress: watch::channel(Ok(origin)).0,
                signal: watch::channel(false).0,
            },
        }
    }
}

impl<A: Application> Capture<A> {
    pub(crate) async fn snapshot(
        &self,
        tracked: &super::TrackedState<A>,
        undos: &super::Undos<A>,
        receipts: &HashMap<RequestKey, Receipt>,
        final_save: bool,
    ) {
        if let Self::Managed {
            storage,
            snapshots: Some(snapshots),
            ..
        } = self
        {
            snapshots
                .capture(storage, tracked, undos, receipts, final_save)
                .await;
        }
    }

    pub(crate) fn take_receipts(&mut self) -> HashMap<RequestKey, Receipt> {
        match self {
            Self::Callback(_) => HashMap::new(),
            Self::Managed { receipts, .. } => std::mem::take(receipts),
        }
    }

    pub(crate) async fn reserve(
        &self,
        bytes: usize,
        group: &crate::state::recovery::Group<&serde_json::Value>,
    ) -> Result<Permit, PreparationError> {
        match self {
            Self::Callback(callback) => callback.reserve(bytes).await.map(Permit::Callback),
            Self::Managed {
                storage,
                projection,
                ..
            } => storage
                .reserve_group(group, projection.as_ref())
                .await
                .map(Permit::Managed)
                .map_err(|error| match error {
                    crate::Error::InvalidInput { .. } => PreparationError::Limit,
                    _ => {
                        self.fail();
                        PreparationError::CaptureFailed
                    }
                }),
        }
    }

    pub(crate) fn check(&self) -> Result<(), PreparationError> {
        match self {
            Self::Callback(callback) => callback.check(),
            Self::Managed { storage, .. } => {
                let status = storage.status();
                if status.failure.is_some() || status.closing || !status.running {
                    self.fail();
                    Err(PreparationError::CaptureFailed)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn submit(
        &self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        frontier: Position,
        permit: Permit,
    ) {
        match (self, permit) {
            (Self::Callback(callback), Permit::Callback(permit)) => {
                callback.submit(entries, frontier, permit)
            }
            (Self::Managed { progress, .. }, Permit::Managed(permit)) => {
                permit.commit(frontier);
                let _ = progress.send_replace(Ok(frontier));
            }
            _ => unreachable!("capture owns its reservation"),
        }
    }

    pub(crate) fn progress(&self) -> watch::Receiver<Result<Position, CaptureError>> {
        match self {
            Self::Callback(callback) => callback.progress(),
            Self::Managed { progress, .. } => progress.subscribe(),
        }
    }

    pub(crate) fn failure(&self) -> watch::Receiver<bool> {
        match self {
            Self::Callback(callback) => callback.failure(),
            Self::Managed { signal, .. } => signal.subscribe(),
        }
    }

    fn fail(&self) {
        if let Self::Managed {
            failed,
            progress,
            signal,
            ..
        } = self
        {
            failed.store(true, Ordering::Release);
            let _ = progress.send_replace(Err(CaptureError::Failed));
            signal.send_replace(true);
        }
    }

    pub(crate) async fn finish(self) {
        if let Self::Callback(callback) = self {
            callback.finish().await;
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
