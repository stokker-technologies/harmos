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
use std::sync::Arc;

// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;

// Local Imports
use super::{InputEnvelope, Opening, Pending};
use crate::Error;
use crate::operations::{OperationInstance, StreamItem, StreamOutput};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("operation failed while forwarding input")]
    Operation(#[source] Box<Error>),

    #[error("input forwarding failed")]
    Forward {
        #[source]
        source: Box<Error>,

        settlement: Option<Box<Error>>,
    },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn open<R, I, S>(
    incoming: S,
) -> Result<(StreamItem<R>, impl Stream<Item = Result<StreamItem<I>, tonic::Status>> + Send), tonic::Status>
where
    R: Send + crate::MemorySize,
    I: Send + crate::MemorySize,
    S: Stream<Item = Result<StreamItem<InputEnvelope<R, I>>, tonic::Status>> + Send,
{
    let mut incoming = Box::pin(incoming);
    let mut opening = Opening::default();
    let first = incoming.next().await.transpose()?;
    let Some(first) = first else {
        return Err(opening.finish(Ok(())).expect_err("no opening was received"));
    };
    let first = opening.validate(InputEnvelope::project(first).map_err(super::server::status)?)?;
    let InputEnvelope::Request(request) = first else {
        unreachable!("validation rejects items before the opening");
    };
    let items = futures_util::stream::unfold(Some((incoming, opening)), |state| async move {
        let (mut incoming, mut opening) = state?;
        let result = match incoming.next().await {
            Some(Ok(frame)) => InputEnvelope::project(frame).map_err(super::server::status).and_then(|frame| opening.validate(frame)),
            Some(Err(error)) => Err(error),
            None => return opening.finish(Ok(())).err().map(|error| (Err(error), None)),
        };
        match result {
            Ok(InputEnvelope::Item(item)) => Some((Ok(item), Some((incoming, opening)))),
            Ok(InputEnvelope::Request(_)) => unreachable!("validation rejects duplicate openings"),
            Err(error) => Some((Err(error), None)),
        }
    });
    Ok((request, items))
}

pub fn input<O, S>(
    instance: OperationInstance<O>, output: StreamOutput<<O::Input as crate::operations::Direction>::Item>, incoming: S,
) -> impl Future<Output = Result<crate::operations::Response<O::Response>, InputError>> + Send
where
    O: crate::operations::OperationDeclaration,
    S: Stream<Item = Result<StreamItem<<O::Input as crate::operations::Direction>::Item>, tonic::Status>> + Send,
{
    let pending = Pending { instance, delivered: false };
    async move {
        let mut pending = pending;
        let forwarded = {
            let forward = async move {
                tokio::pin!(incoming);
                while let Some(item) = incoming.next().await {
                    let item = item.map_err(|source| Error::TaskFailed { domain: "gRPC input", source: Arc::new(source) })?;
                    output.send_item(item).await?;
                }
                Ok::<(), Error>(())
            };
            tokio::pin!(forward);
            tokio::select! {
                biased;
                result = &mut pending.instance => {
                    pending.delivered = true;
                    return result.map_err(|error| InputError::Operation(Box::new(error)));
                }
                result = &mut forward => result,
            }
        };
        if forwarded.is_err() {
            pending.instance.cancel();
        }
        let result = (&mut pending.instance).await;
        pending.delivered = true;
        match forwarded {
            Ok(()) => result.map_err(|error| InputError::Operation(Box::new(error))),
            Err(source) => Err(InputError::Forward { source: Box::new(source), settlement: result.err().map(Box::new) }),
        }
    }
}

/* === Private Functions === */

/* === Tests === */
