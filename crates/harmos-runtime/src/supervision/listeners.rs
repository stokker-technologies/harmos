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
use std::any::Any;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
// Third Party Imports
use tokio::sync::{Mutex as AsyncMutex, watch};
// Local Imports
use super::Task;
use crate::operations::EncodedInvocation;
use crate::plugin::PluginScope;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Listener {
    scope: PluginScope,
    task: Task<()>,
    delivery: Arc<Delivery>,
    cleaned: AtomicBool,
}

pub(crate) struct Delivery {
    pending: AsyncMutex<Option<Pending>>,
    failure: Mutex<Option<Arc<Error>>>,
}

struct Pending {
    invocation: EncodedInvocation,
    // Same Original Source Allocation, Held Through Physical Settlement
    _source: Arc<dyn Any + Send + Sync>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Listener {
    pub(crate) fn spawn<F>(scope: &PluginScope, run: impl FnOnce(Arc<Delivery>, watch::Receiver<bool>) -> F + Send + 'static) -> Self
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        let delivery = Arc::new(Delivery {
            pending: AsyncMutex::new(None),
            failure: Mutex::new(None),
        });
        let task = Task::spawn({
            let delivery = Arc::clone(&delivery);
            let scope = scope.clone();
            move |stopping| async move {
                let mut running = Box::pin(run(Arc::clone(&delivery), stopping.subscribe()));
                let answer = std::future::poll_fn(|context| {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| running.as_mut().poll(context))) {
                        Ok(poll) => poll,
                        Err(_) => Poll::Ready(Err(Error::OperationPanicked { operation: scope.owner().to_owned() })),
                    }
                })
                .await;
                // Drop A Suspended Delivery Borrow Before Cancelling Its Receipt.
                drop(running);
                if let Err(error) = answer {
                    *delivery.failure.lock().expect("listener failure poisoned") = Some(Arc::new(error));
                    scope.close_listeners();
                    if let Some(original) = delivery.pending.lock().await.as_ref() {
                        original.invocation.request_cancel();
                    }
                }
            }
        });
        Self {
            scope: scope.clone(),
            task,
            delivery,
            cleaned: AtomicBool::new(false),
        }
    }

    pub(crate) fn scope(&self) -> &PluginScope {
        &self.scope
    }

    pub(crate) fn failure(&self) -> Option<Arc<Error>> {
        self.delivery.failure.lock().expect("listener failure poisoned").clone()
    }

    pub(crate) fn cleaned(&self) -> bool {
        self.cleaned.load(Ordering::Acquire)
    }

    pub(crate) fn stop(&self) {
        self.scope.close_listeners();
        self.task.stop();
    }

    pub(crate) async fn shutdown(&self) -> Result<()> {
        self.stop();
        self.task.settle().await.map_err(|source| Error::OperationFailed {
            operation: self.scope.owner().to_owned(),
            source,
        })?;
        let mut pending = self.delivery.pending.lock().await;
        if let Some(original) = pending.as_ref() {
            original.invocation.request_cancel();
            original.invocation.control.settle().await.map_err(|source| Error::OperationSettlement { source })?;
        }
        // Same Positive Physical Control Before Source Release; Failed Wait Keeps It.
        pending.take();
        self.cleaned.store(true, Ordering::Release);
        Ok(())
    }
}

impl Delivery {
    pub(crate) async fn invoke(
        &self, invocation: EncodedInvocation, source: Arc<dyn Any + Send + Sync>, scope: &PluginScope, stopping: &mut watch::Receiver<bool>,
    ) -> Result<bool> {
        let control = Arc::clone(&invocation.control);
        let mut original = self.pending.lock().await;
        assert!(original.is_none(), "one original callback per listener task");
        *original = Some(Pending { invocation, _source: source });
        let invocation = &mut original.as_mut().expect("retained callback").invocation;
        let outcome = tokio::select! {
            biased;
            _ = stopping.changed() => { invocation.request_cancel(); return Ok(false); },
            _ = scope.closed() => { invocation.request_cancel(); return Ok(false); },
            outcome = &mut *invocation => outcome,
        };
        match outcome {
            Ok(_) => control.settle().await.map_err(|source| Error::OperationSettlement { source })?,
            Err(error) => {
                if control.is_settled() {
                    original.take();
                }
                return Err(error);
            }
        }
        original.take();
        Ok(true)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
