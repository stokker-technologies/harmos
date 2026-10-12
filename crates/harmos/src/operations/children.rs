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
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::Error;
use crate::operations::{EncodedInvocation, WireChildCall};
use harmos_contract::Refusal;

/* === Constants === */
pub(crate) const CHILD_REQUESTS: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub(crate) struct Children {
    pub(crate) context: [u8; 32],
    pub(crate) requests: mpsc::Sender<ChildRequest>,
}

pub(crate) struct ChildRequest {
    pub(crate) operation: String,
    pub(crate) payload: Vec<u8>,
    pub(crate) submitted: tokio::time::Instant,
    pub(crate) answer: oneshot::Sender<Result<EncodedInvocation, Error>>,
}

struct Child {
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
        let mut invocation = Child {
            invocation: Some(invocation),
        };
        // Cancellation Requests Interruption; The Same Child Still Drains
        tokio::select! {
            biased;
            _ = &mut cancelled => {
                invocation.invocation.as_ref().expect("owned child").cancel();
                invocation.await.map_err(refused)
            }
            outcome = &mut invocation => outcome.map_err(refused),
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
        Error::OperationFailed { ref source, .. } => {
            let refusal = source.downcast_ref::<Refusal>().cloned();
            refusal.unwrap_or_else(|| Refusal::of(error))
        }
        Error::OperationUnauthorized { .. } | Error::OperationLaneCycle { .. } => {
            Refusal::new(-32_604, error)
        }
        Error::NotRegistered { .. } => Refusal::new(harmos_contract::UNKNOWN, error),
        Error::OperationPayload { .. } => Refusal::new(harmos_contract::UNREADABLE, error),
        Error::OperationRefused(crate::Refusal::TimedOut) => Refusal::new(-32_607, error),
        Error::OperationRefused(_) | Error::Closing { .. } => Refusal::new(-32_606, error),
        error => Refusal::of(error),
    }
}

/* === Private Functions === */

/* === Tests === */
