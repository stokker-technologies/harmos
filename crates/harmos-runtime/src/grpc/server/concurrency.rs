/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use http_body::{Frame, SizeHint};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tonic::{Status, body::Body};

use super::HttpFuture;

#[derive(Clone)]
pub struct Calls {
    slots: Arc<Semaphore>,
}

struct Retained {
    body: Body,
    permit: Option<OwnedSemaphorePermit>,
}

impl Calls {
    pub fn new(limit: NonZeroUsize) -> Result<Self, Status> {
        Self::validate(limit)?;
        Ok(Self {
            slots: Arc::new(Semaphore::new(limit.get())),
        })
    }

    pub(super) fn validate(limit: NonZeroUsize) -> Result<(), Status> {
        if limit.get() > Semaphore::MAX_PERMITS {
            return Err(Status::invalid_argument("gRPC call limit exceeds supported capacity"));
        }
        Ok(())
    }

    pub fn run(&self, call: HttpFuture) -> HttpFuture {
        let permit = self.slots.clone().try_acquire_owned();
        Box::pin(async move {
            let Ok(permit) = permit else {
                return Status::resource_exhausted("gRPC call capacity exhausted").into_http();
            };
            let response = call.await;
            response.map(|body| Body::new(Retained { body, permit: Some(permit) }))
        })
    }
}

impl http_body::Body for Retained {
    type Data = bytes::Bytes;
    type Error = Status;

    fn poll_frame(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = Pin::new(&mut self.body).poll_frame(cx);
        if matches!(frame, Poll::Ready(None | Some(Err(_)))) || self.body.is_end_stream() {
            self.permit.take();
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}
