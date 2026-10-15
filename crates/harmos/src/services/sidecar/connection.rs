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
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};
// Local Imports
use super::{Arrival, Children, Request, SidecarError};
use crate::operations::{Contract, Definition, WIRE_METHOD, WireAnswer, WireCommand};
use harmos_sidecar::Frame;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Endpoint {
    provider: Option<Arc<crate::supervision::instance::Instance>>,
    requests: mpsc::UnboundedSender<Request>,
    arrivals: mpsc::Sender<Arrival>,
    sequences: Mutex<BTreeMap<String, u64>>,
    turns: Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>,
    capacity: Mutex<Option<(NonZeroUsize, Arc<Semaphore>)>>,
    operations: Vec<Definition>,
    contracts: Vec<Contract>,
    plugin: Option<crate::plugin::PluginScope>,
    observers: Option<Weak<super::observers::Remote>>,
    observer_outcome: Option<Arc<Mutex<Option<Arc<crate::Error>>>>>,
    accepting: AtomicBool,
    initialized: watch::Sender<bool>,
    ended: watch::Receiver<bool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Endpoint {
    pub(super) fn new(
        requests: mpsc::UnboundedSender<Request>,
        arrivals: mpsc::Sender<Arrival>,
        operations: &[Definition],
        contracts: &[Contract],
        ended: watch::Receiver<bool>,
        plugin: Option<crate::plugin::PluginScope>,
    ) -> Self {
        Self {
            provider: None,
            requests,
            arrivals,
            sequences: Mutex::new(BTreeMap::new()),
            turns: Mutex::new(BTreeMap::new()),
            capacity: Mutex::new(None),
            operations: operations.to_vec(),
            contracts: contracts.to_vec(),
            initialized: watch::channel(
                plugin
                    .as_ref()
                    .is_none_or(|scope| scope.lifecycle().is_none()),
            )
            .0,
            plugin,
            observers: None,
            observer_outcome: None,
            accepting: AtomicBool::new(true),
            ended,
        }
    }

    pub(super) fn with_observers(mut self, observers: &Arc<super::observers::Remote>) -> Self {
        self.provider = observers.provider.clone();
        self.observers = Some(Arc::downgrade(observers));
        self.observer_outcome = Some(observers.outcome());
        self
    }

    pub(crate) fn retain_provider(
        &self,
        dependent: &Arc<crate::supervision::instance::Instance>,
        retirement: bool,
    ) -> crate::Result<()> {
        if let Some(provider) = &self.provider {
            dependent.depend_on(provider, retirement)?;
        }
        Ok(())
    }

    pub(crate) async fn settle_observers(&self) -> crate::Result<()> {
        if self.accepts()
            && let Some(original) = self.observers.as_ref().and_then(Weak::upgrade)
        {
            original.settle().await?;
        }
        Ok(())
    }

    pub(crate) fn observer_outcome(&self) -> crate::Result<()> {
        let outcome = self
            .observer_outcome
            .as_ref()
            .and_then(|outcome| outcome.lock().expect("observer outcome is healthy").clone());
        match outcome {
            Some(source) => Err(crate::Error::TaskFailed {
                domain: "remote observer",
                source,
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn capture_observer_control(&self) -> crate::Result<()> {
        if !self
            .plugin
            .as_ref()
            .is_some_and(|scope| scope.observer_control().is_some())
        {
            return Ok(());
        }
        let original =
            self.observers
                .as_ref()
                .and_then(Weak::upgrade)
                .ok_or(crate::Error::Closing {
                    domain: "native observer owner",
                })?;
        original.capture_control()
    }

    pub(crate) fn bind_capacity(&self, limit: NonZeroUsize) -> crate::Result<Arc<Semaphore>> {
        if limit.get() > Semaphore::MAX_PERMITS {
            return Err(crate::Error::InvalidInput {
                field: "native operation capacity",
                reason: "capacity exceeds the semaphore limit",
            });
        }
        let mut capacity = self
            .capacity
            .lock()
            .expect("native operation capacity poisoned");
        if let Some((original, semaphore)) = capacity.as_ref() {
            if *original != limit {
                return Err(crate::Error::InvalidInput {
                    field: "native operation capacity",
                    reason: "capacity differs from the original generation binding",
                });
            }
            return Ok(Arc::clone(semaphore));
        }
        let semaphore = Arc::new(Semaphore::new(limit.get()));
        *capacity = Some((limit, Arc::clone(&semaphore)));
        Ok(semaphore)
    }

    pub(crate) fn reserve_capacity(&self) -> Option<OwnedSemaphorePermit> {
        let semaphore = {
            let capacity = self
                .capacity
                .lock()
                .expect("native operation capacity poisoned");
            Arc::clone(&capacity.as_ref()?.1)
        };
        semaphore.try_acquire_owned().ok()
    }

    pub(crate) fn definitions(&self) -> &[Definition] {
        &self.operations
    }

    pub(crate) fn contracts(&self) -> &[Contract] {
        &self.contracts
    }

    pub(crate) fn scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.plugin.as_ref()
    }

    pub(super) async fn publish(
        &self,
        scope: &crate::plugin::PluginScope,
        request: &mut crate::operations::host::Publication,
        context: &crate::operations::Context<'_>,
    ) -> crate::Result<()> {
        // Calls On One Stream Stage In Arrival Order
        let turn = self
            .turns
            .lock()
            .expect("native publication turns poisoned")
            .entry(request.stream.clone())
            .or_default()
            .clone();
        let _turn = turn.lock().await;
        let lease = scope.publication_in(context, &request.stream, request.schema)?;

        // Every Row Published Before This Call Is Ingested Before It Stages
        let mut ended = self.ended.clone();
        tokio::select! {
            ingested = lease.ingested() => ingested.map_err(|source| crate::Error::OperationFailed {
                operation: "host.publish".into(),
                source: Arc::new(source),
            })?,
            _ = ended.wait_for(|ended| *ended) => {
                return Err(crate::Error::Closing {
                    domain: "native publication connection",
                });
            }
        }

        let permit = self
            .arrivals
            .reserve()
            .await
            .map_err(|_| crate::Error::Closing {
                domain: "native publication delivery",
            })?;
        if !self.accepts()
            || !self
                .plugin
                .as_ref()
                .is_some_and(|original| original.same_activation(scope))
        {
            return Err(crate::Error::Closing {
                domain: "native publication connection",
            });
        }
        let first = request.sequence.ok_or(crate::Error::InvalidInput {
            field: "native publication",
            reason: "the original producer sequence is required",
        })?;
        let rows = 1 + request.following.len() as u64;
        first
            .checked_add(rows - 1)
            .ok_or(crate::Error::InvalidInput {
                field: "native publication",
                reason: "the producer sequence must not overflow",
            })?;
        let head = (
            std::mem::take(&mut request.payload),
            request.published_unix_micros,
        );
        // Each Following Delta Rebuilds A Row's Stamp From Its Predecessor's.
        let mut previous = request.published_unix_micros;
        let mut following = Vec::with_capacity(request.following.len());
        for row in std::mem::take(&mut request.following) {
            let stamp = match (previous, row.published_delta_micros) {
                (_, None) => None,
                (Some(previous), Some(delta)) => Some(previous.checked_add(delta).ok_or(
                    crate::Error::InvalidInput {
                        field: "native publication",
                        reason: "a row stamp must not overflow",
                    },
                )?),
                (None, Some(_)) => {
                    return Err(crate::Error::InvalidInput {
                        field: "native publication",
                        reason: "a stamp delta requires a stamped preceding row",
                    });
                }
            };
            previous = stamp;
            following.push((row.payload, stamp));
        }
        let mut batch: Vec<_> = std::iter::once(head)
            .chain(following)
            .map(
                |(payload, published_unix_micros)| crate::streams::PublicationRow {
                    payload,
                    published_unix_micros,
                    context: None,
                },
            )
            .collect();
        // Every Row Carries The Call Context; The Last Row Takes The Original.
        if let Some((last, rest)) = batch.split_last_mut() {
            for row in rest {
                row.context.clone_from(&request.context);
            }
            last.context = request.context.take();
        }
        let staged = {
            let mut sequences = self
                .sequences
                .lock()
                .expect("native publication sequence poisoned");
            let previous = sequences.get(&request.stream).copied().unwrap_or(0);
            let missed = first
                .checked_sub(previous)
                .and_then(|advance| advance.checked_sub(1))
                .ok_or(crate::Error::InvalidInput {
                    field: "native publication",
                    reason: "the producer sequence must strictly advance",
                })?;
            let staged = lease.stage_rows_in(context, batch, first, missed);
            // Rows Before A Refused Row Stay Staged; The Rest Count As Missed.
            if let Some(advanced) = (staged.staged.len() as u64).checked_sub(1) {
                sequences.insert(request.stream.clone(), first + advanced);
            }
            staged
        };
        if !staged.staged.is_empty() {
            permit.send(Arrival::Publications(staged.staged));
        }
        match staged.refused {
            Some(source) => Err(crate::Error::OperationFailed {
                operation: "host.publish".into(),
                source: Arc::new(source),
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn initialized(&self) -> watch::Receiver<bool> {
        self.initialized.subscribe()
    }

    pub(crate) fn acknowledge_initialize(&self) {
        self.initialized.send_replace(true);
    }

    pub(crate) fn accepts(&self) -> bool {
        self.accepting.load(Ordering::Acquire)
    }

    pub(super) fn close(&self) {
        self.accepting.store(false, Ordering::Release);
        if let Some(scope) = &self.plugin
            && scope.lifecycle().is_some()
        {
            scope.close();
        }
    }

    pub(super) fn send(&self, request: Request) -> Result<(), SidecarError> {
        if !self.accepts() {
            return Err(SidecarError::Unavailable);
        }
        self.requests
            .send(request)
            .map_err(|_| SidecarError::Unavailable)
    }

    pub(crate) async fn phase(&self, command: WireCommand) -> Result<WireAnswer, SidecarError> {
        self.request(command, None).await
    }

    pub(crate) async fn attempt(
        &self,
        command: WireCommand,
        children: Children,
    ) -> Result<WireAnswer, SidecarError> {
        self.request(command, Some(children)).await
    }

    async fn request(
        &self,
        command: WireCommand,
        children: Option<Children>,
    ) -> Result<WireAnswer, SidecarError> {
        if !self.accepts() {
            return Err(SidecarError::Unavailable);
        }
        let params = command.encode().map_err(|error| match error {
            harmos_contract::operations::WireCodecError::Oversized => SidecarError::Refused {
                code: Frame::OVERSIZED,
                message: "operation control exceeds the native frame limit".to_owned(),
            },
            error => SidecarError::Malformed(error.to_string()),
        })?;
        let frame = Frame::Call {
            id: u64::MAX,
            method: WIRE_METHOD.to_owned(),
            params,
        };
        if !frame.fits() {
            return Err(SidecarError::Refused {
                code: Frame::OVERSIZED,
                message: "operation control exceeds the native frame limit".to_owned(),
            });
        }
        let Frame::Call { method, params, .. } = frame else {
            unreachable!()
        };
        let (answer, answered) = oneshot::channel();
        let request = Request {
            method,
            params,
            answer,
            children,
        };
        self.send(request)?;
        let payload = answered.await.map_err(|_| SidecarError::Unavailable)??;
        WireAnswer::decode(&payload).map_err(|error| SidecarError::Malformed(error.to_string()))
    }

    pub(crate) async fn settle(&self) -> Result<(), SidecarError> {
        let mut ended = self.ended.clone();
        while !*ended.borrow_and_update() {
            ended
                .changed()
                .await
                .map_err(|_| SidecarError::Unavailable)?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
