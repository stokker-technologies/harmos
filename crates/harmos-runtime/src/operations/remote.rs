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
use std::sync::atomic::{AtomicBool, Ordering};

// Third Party Imports
use tokio::sync::watch;

// Local Imports
use super::{EndpointSet, StreamInput, StreamOutput};
use crate::operations::{FailureKind, WireAnswer, WireCommand};
use crate::{MemorySize, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Transfer<'a> {
    observed: &'a AtomicBool,
    completed: bool,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum RemoteStreamError {
    #[error("remote stream protocol failed: {0}")]
    Protocol(&'static str),

    #[error("remote stream item codec failed")]
    Payload(#[from] crate::MessageError),

    #[error("remote stream output admission failed")]
    Output(#[from] crate::Error),

    // The provider's stream failure crosses unchanged.
    #[error(transparent)]
    Remote(harmos_domain::Failure),
}

/* === Traits === */

pub trait StreamPeer: Sync {
    type Error: From<RemoteStreamError> + Send;
    fn stream_phase(&self, command: WireCommand) -> impl Future<Output = Result<WireAnswer, Self::Error>> + Send;
}

pub trait StreamEndpoints: EndpointSet {
    fn pump<P: StreamPeer>(
        &mut self, peer: &P, invocation: u64, epoch: u64, ended: watch::Receiver<bool>, transferred: &AtomicBool,
    ) -> impl Future<Output = Result<(), P::Error>> + Send;
}

/* === Implementations === */

impl Drop for Transfer<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.observed.store(true, Ordering::Release);
        }
    }
}
impl RemoteStreamError {
    pub(super) fn classification(&self) -> FailureKind {
        match self {
            Self::Protocol(_) | Self::Payload(_) => FailureKind::Invalid,
            Self::Remote(failure) => failure.code(),
            Self::Output(_) => FailureKind::Permanent,
        }
    }

    pub(super) fn failure(&self) -> Option<&harmos_domain::Failure> {
        match self {
            Self::Remote(failure) => Some(failure),
            _ => None,
        }
    }

    pub(super) fn in_operation(self, operation: &str) -> Self {
        match self {
            Self::Remote(failure) => Self::Remote(failure.in_operation(operation)),
            error => error,
        }
    }
}
impl StreamEndpoints for () {
    #[tracing::instrument(target = "harmos::operations", name = "operations.pump", level = "debug", skip_all)]
    async fn pump<P: StreamPeer>(&mut self, _: &P, _: u64, _: u64, _: watch::Receiver<bool>, _: &AtomicBool) -> Result<(), P::Error> {
        Ok(())
    }
}
impl<T: Message + MemorySize + 'static> StreamEndpoints for StreamInput<T> {
    #[tracing::instrument(target = "harmos::operations", name = "operations.pump", level = "debug", skip_all)]
    async fn pump<P: StreamPeer>(
        &mut self, peer: &P, invocation: u64, epoch: u64, mut ended: watch::Receiver<bool>, _: &AtomicBool,
    ) -> Result<(), P::Error> {
        let sending = async {
            while let Some(item) = self.recv().await {
                if item.encoded_size() > WireCommand::LIMIT {
                    return Err(RemoteStreamError::Protocol("stream item exceeds frame limit").into());
                }
                let command = WireCommand::StreamSend { invocation, epoch, payload: item.encode() };
                acknowledge(peer.stream_phase(command).await?).map_err(P::Error::from)?;
                // The Original Item Claim Is Retained Until Provider Admission.
                drop(item);
            }
            acknowledge(peer.stream_phase(WireCommand::StreamFinish { invocation, epoch }).await?).map_err(P::Error::from)
        };
        tokio::select! {
            biased;
            _ = ended.wait_for(|ended| *ended) => Ok(()),
            result = sending => result,
        }
    }
}
impl<T: Message + MemorySize + 'static> StreamEndpoints for StreamOutput<T> {
    #[tracing::instrument(target = "harmos::operations", name = "operations.pump", level = "debug", skip_all)]
    async fn pump<P: StreamPeer>(
        &mut self, peer: &P, invocation: u64, epoch: u64, _: watch::Receiver<bool>, transferred: &AtomicBool,
    ) -> Result<(), P::Error> {
        loop {
            let answer = peer.stream_phase(WireCommand::StreamReceive { invocation, epoch }).await?;
            let payload = match answer {
                WireAnswer::Stream { epoch: received, transferred: observed, payload } if received == epoch => {
                    if observed {
                        transferred.store(true, Ordering::Release);
                    }
                    payload
                }
                WireAnswer::Failed { failure } => {
                    return Err(RemoteStreamError::Remote(failure).into());
                }
                _ => {
                    return Err(RemoteStreamError::Protocol("expected stream reply for the original attempt").into());
                }
            };
            let Some(payload) = payload else {
                return Ok(());
            };
            if payload.capacity() > WireCommand::LIMIT {
                return Err(RemoteStreamError::Protocol("stream response exceeds frame limit").into());
            }
            // Actual Items Fence Retry Even If A Malformed Peer Omits Its Flag.
            transferred.store(true, Ordering::Release);
            self.decode(&payload).await.map_err(RemoteStreamError::Output).map_err(P::Error::from)?;
        }
    }
}
impl<I, O> StreamEndpoints for (StreamInput<I>, StreamOutput<O>)
where
    I: Message + MemorySize + 'static,
    O: Message + MemorySize + 'static,
{
    #[tracing::instrument(target = "harmos::operations", name = "operations.pump", level = "debug", skip_all)]
    async fn pump<P: StreamPeer>(
        &mut self, peer: &P, invocation: u64, epoch: u64, ended: watch::Receiver<bool>, transferred: &AtomicBool,
    ) -> Result<(), P::Error> {
        tokio::try_join!(
            self.0.pump(peer, invocation, epoch, ended.clone(), transferred),
            self.1.pump(peer, invocation, epoch, ended, transferred),
        )?;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tracing::instrument(target = "harmos::carrier", name = "carrier.stream_attempt", level = "debug", skip_all, fields(invocation, epoch))]
pub(super) async fn attempt<P: StreamPeer, E: StreamEndpoints>(
    peer: &P, endpoints: &mut E, invocation: u64, epoch: u64, transferred: &AtomicBool,
    running: impl Future<Output = Result<WireAnswer, P::Error>> + Send,
) -> Result<WireAnswer, P::Error> {
    let mut transfer = Transfer { observed: transferred, completed: false };
    let (stop, ended) = watch::channel(false);
    let pumping = endpoints.pump(peer, invocation, epoch, ended, transferred);
    tokio::pin!(running, pumping);
    let answer = tokio::select! {
        biased;
        result = &mut running => {
            let answer = result?;
            stop.send_replace(true);
            // Input Can End Early; Every Produced Output Must Still Be Drained.
            pumping.await?;
            answer
        }
        result = &mut pumping => {
            result?;
            running.await?
        }
    };
    transfer.completed = true;
    Ok(answer)
}

fn acknowledge(answer: WireAnswer) -> Result<(), RemoteStreamError> {
    match answer {
        WireAnswer::Ready { payload } if payload.is_empty() => Ok(()),
        WireAnswer::Failed { failure } => Err(RemoteStreamError::Remote(failure)),
        _ => Err(RemoteStreamError::Protocol("expected empty stream acknowledgement")),
    }
}

/* === Tests === */
