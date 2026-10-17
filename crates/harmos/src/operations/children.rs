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
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::Error;
use crate::operations::{EncodedInvocation, WireChildAction, WireChildCall, WireChildControl};
use harmos_domain::{Message, Refusal};

/* === Constants === */
pub(crate) const CHILD_REQUESTS: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub(crate) struct Children {
    pub(crate) context: [u8; 32],
    pub(crate) requests: mpsc::Sender<ChildRequest>,
    // Submitted children this attempt still owns; dropping one cancels it.
    pub(crate) submitted: Arc<Mutex<BTreeMap<u64, Child>>>,
}

pub(crate) struct ChildRequest {
    pub(crate) operation: String,
    pub(crate) payload: Vec<u8>,
    pub(crate) values: Vec<super::CallValue>,
    pub(crate) submitted: tokio::time::Instant,
    pub(crate) answer: oneshot::Sender<Result<EncodedInvocation, Error>>,
}

pub(crate) struct Child {
    invocation: Option<EncodedInvocation>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Children {
    pub(crate) async fn call(
        &self,
        call: WireChildCall,
        submitted: tokio::time::Instant,
        mut cancelled: oneshot::Receiver<()>,
    ) -> Result<Vec<u8>, Refusal> {
        let (answer, answered) = oneshot::channel();
        let request = ChildRequest {
            operation: call.operation,
            payload: call.payload,
            values: call.values,
            submitted,
            answer,
        };
        let admission = async {
            self.requests
                .send(request)
                .await
                .map_err(|_| Error::Closing {
                    domain: "carrier attempt",
                })?;
            answered.await.map_err(|_| Error::Closing {
                domain: "carrier attempt",
            })?
        };
        let invocation = tokio::select! {
            biased;
            _ = &mut cancelled => {
                let error: Error = Error::OperationRefused(crate::Refusal::Cancelled);
                return Err(refused(error));
            }
            admitted = admission => admitted.map_err(refused)?,
        };
        let invocation = Child {
            invocation: Some(invocation),
        };
        if call.submit {
            let id = invocation.invocation.as_ref().expect("owned child").id();
            self.submitted
                .lock()
                .expect("submitted child lock is healthy")
                .insert(id.value(), invocation);
            return Ok(id.value().encode_to_vec());
        }
        drained(invocation, cancelled).await
    }

    pub(crate) async fn control(
        &self,
        control: WireChildControl,
        cancelled: oneshot::Receiver<()>,
    ) -> Result<Vec<u8>, Refusal> {
        let instance = control.instance;
        let take = || {
            self.submitted
                .lock()
                .expect("submitted child lock is healthy")
                .remove(&instance)
                .ok_or_else(|| unknown(instance))
        };
        match control.action {
            WireChildAction::Join => drained(take()?, cancelled).await,
            WireChildAction::Status | WireChildAction::Cancel => {
                let submitted = self
                    .submitted
                    .lock()
                    .expect("submitted child lock is healthy");
                let invocation = submitted
                    .get(&instance)
                    .and_then(|child| child.invocation.as_ref())
                    .ok_or_else(|| unknown(instance))?;
                if control.action == WireChildAction::Cancel {
                    invocation.cancel();
                }
                let state = harmos_domain::operations::lifecycle::InstanceState::from(
                    invocation.control.status().portable(),
                );
                Ok(state.encode_to_vec())
            }
            WireChildAction::Release => {
                drop(take()?);
                Ok(Vec::new())
            }
            WireChildAction::Detach => {
                let mut child = take()?;
                // Without Its Handle The Child Runs On; Its Scope Still Settles It
                drop(child.invocation.take());
                Ok(Vec::new())
            }
        }
    }
}

impl Future for Child {
    type Output = crate::Result<Vec<u8>>;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let result = Pin::new(self.invocation.as_mut().expect("owned child")).poll(context);
        if result.is_ready() {
            self.invocation.take();
        }
        result
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if let Some(invocation) = &self.invocation {
            invocation.cancel();
        }
    }
}

/* === Public Functions === */

#[cfg(any(feature = "plugin-sidecar", feature = "plugin-guest"))]
pub(crate) async fn lifecycle(
    invocation: EncodedInvocation,
    mut cancelled: oneshot::Receiver<()>,
) -> crate::Result<Vec<u8>> {
    let control = std::sync::Arc::clone(&invocation.control);
    let mut invocation = Child {
        invocation: Some(invocation),
    };
    let outcome = tokio::select! {
        biased;
        _ = &mut cancelled => {
            invocation.invocation.as_ref().expect("owned lifecycle invocation").cancel();
            invocation.await
        }
        outcome = &mut invocation => outcome,
    };
    control
        .settle()
        .await
        .map_err(|source| Error::OperationSettlement { source })?;
    outcome
}

pub(crate) fn refused(error: Error) -> Refusal {
    match error {
        Error::OperationCaller { source } => source.into_refusal(),
        Error::OperationFailed {
            ref operation,
            ref source,
        } => {
            if let Some(refusal) = source.downcast_ref::<Refusal>() {
                let failure = refusal.failure().clone().in_operation(operation.as_str());
                return Refusal::failed(refusal.code(), failure);
            }
            // A Carried Failure Keeps Its Cause And Any Frame Closer To The Origin
            let failure = error.failure().cloned().unwrap_or_else(|| {
                harmos_domain::Failure::originate(
                    crate::operations::FailureKind::Permanent,
                    &error,
                    harmos_domain::failure::Origin::Host,
                )
            });
            let failure = failure.in_operation(operation.as_str());
            Refusal::failed(harmos_domain::FAILED, failure)
        }
        Error::OperationUnauthorized { .. } | Error::OperationLaneCycle { .. } => {
            Refusal::new(-32_604, error)
        }
        Error::NotRegistered { .. } => Refusal::new(harmos_domain::UNKNOWN, error),
        Error::OperationPayload { .. } => Refusal::new(harmos_domain::UNREADABLE, error),
        Error::OperationRefused(crate::Refusal::TimedOut) => Refusal::new(-32_607, error),
        Error::OperationRefused(_) | Error::Closing { .. } => Refusal::new(-32_606, error),
        error => Refusal::originate(
            harmos_domain::FAILED,
            crate::operations::FailureKind::Permanent,
            &error,
            harmos_domain::failure::Origin::Host,
        ),
    }
}

/* === Private Functions === */
// Await one child; cancellation requests interruption and the same child still
// drains before its outcome is reported.
async fn drained(
    mut invocation: Child,
    mut cancelled: oneshot::Receiver<()>,
) -> Result<Vec<u8>, Refusal> {
    tokio::select! {
        biased;
        _ = &mut cancelled => {
            invocation.invocation.as_ref().expect("owned child").cancel();
            invocation.await.map_err(refused)
        }
        outcome = &mut invocation => outcome.map_err(refused),
    }
}

fn unknown(instance: u64) -> Refusal {
    Refusal::new(
        harmos_domain::UNKNOWN,
        format!("operation instance {instance} is not a child this attempt owns"),
    )
}

/* === Tests === */
