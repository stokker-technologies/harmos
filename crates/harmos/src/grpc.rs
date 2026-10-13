/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod authentication;

mod codec;

mod decoding;
mod envelope;
mod generation;

mod ingress;

mod input;

mod listener;

mod manager;

mod origin;

pub mod middleware;

mod reflection;

pub(crate) mod routing;

pub mod server;

pub mod __private {
    pub use tonic;
}

/* === Re-Exports === */

pub use codec::{Codec, Encode, EncodeSettings, Encoder};

pub use decoding::{DecodeSettings, Decoder};
pub use envelope::{InputEnvelope, OutputEnvelope};
pub use generation::{GenerateError, GeneratedService, Method, generate_server};
pub use harmos_contract::grpc::{Artifacts, DescriptorError, ExportError, descriptor};

pub use input::{InputError, input, open};

pub use manager::{Attached, GrpcManager, Settings};
pub use origin::Origin;

/* === Imports === */

// Standard Library Imports

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

// Third Party Imports

use futures_core::Stream;

use tokio::sync::mpsc;
use tonic::Status;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Settlement<R, E> = Pin<Box<dyn Future<Output = Result<Arc<R>, E>> + Send>>;

/* === Structs === */

pub struct ResponseStream<O: crate::operations::OperationDeclaration, T, E = crate::Error> {
    pending: Settlement<O::Response, E>,
    items: Option<mpsc::Receiver<crate::operations::StreamItem<T>>>,
    result: Option<Result<Arc<O::Response>, E>>,
}

struct Pending<O: crate::operations::OperationDeclaration> {
    instance: crate::operations::OperationInstance<O>,
    delivered: bool,
}

pub struct Completion<R> {
    state: State<R>,
}

#[derive(Default)]
pub struct Opening {
    opened: bool,
    failure: Option<Status>,
}

/* === Enums === */

pub enum Event<T, R> {
    Item(T),
    Completion(R),
}

enum State<R> {
    Open,
    Response(R),
    Failed(Status),
}

/* === Traits === */

/* === Implementations === */

impl<O: crate::operations::OperationDeclaration, T, E: Unpin> Stream for ResponseStream<O, T, E> {
    type Item = Result<Event<crate::operations::StreamItem<T>, Arc<O::Response>>, E>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let stream = self.get_mut();
        let Some(items) = stream.items.as_mut() else {
            return Poll::Ready(None);
        };

        // Poll Settlement Even While Items Are Available.
        if stream.result.is_none()
            && let Poll::Ready(result) = stream.pending.as_mut().poll(context)
        {
            items.close();
            stream.result = Some(result);
        }

        // A Retained Sender Or Permit Cannot Keep A Settled RPC Open.
        if stream.result.is_some() {
            if let Ok(item) = items.try_recv() {
                return Poll::Ready(Some(Ok(Event::Item(item))));
            }
            stream.items.take();
            let result = stream.result.take().expect("original result is ready");
            return Poll::Ready(Some(result.map(Event::Completion)));
        }

        match items.poll_recv(context) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(Ok(Event::Item(item)))),
            // Item EOF Alone Cannot Report Successful Physical Settlement.
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        }
    }
}

impl<O: crate::operations::OperationDeclaration> Drop for Pending<O> {
    fn drop(&mut self) {
        if !self.delivered {
            self.instance.cancel();
        }
    }
}

impl<R> Default for Completion<R> {
    fn default() -> Self {
        Self { state: State::Open }
    }
}

impl<R> Completion<R> {
    pub fn item(&mut self) -> Result<(), Status> {
        match &self.state {
            State::Open => Ok(()),
            State::Failed(error) => Err(error.clone()),
            State::Response(_) => {
                let error = Status::data_loss("output item after final response");
                self.fail(error.clone());
                Err(error)
            }
        }
    }

    pub fn complete(&mut self, response: R) -> Result<(), Status> {
        match &self.state {
            State::Open => {
                self.state = State::Response(response);
                Ok(())
            }
            State::Failed(error) => Err(error.clone()),
            State::Response(_) => {
                let error = Status::data_loss("duplicate final response");
                self.fail(error.clone());
                Err(error)
            }
        }
    }

    pub fn fail(&mut self, error: Status) {
        if !matches!(self.state, State::Failed(_)) {
            self.state = State::Failed(error);
        }
    }

    pub fn finish(self, termination: Result<(), Status>) -> Result<R, Status> {
        match self.state {
            State::Failed(error) => Err(error),
            State::Open => {
                termination?;
                Err(Status::data_loss("missing final response"))
            }
            State::Response(response) => {
                termination?;
                Ok(response)
            }
        }
    }
}

impl Opening {
    fn validate<R, I>(
        &mut self,
        envelope: InputEnvelope<R, I>,
    ) -> Result<InputEnvelope<R, I>, Status> {
        match &envelope {
            InputEnvelope::Request(_) => self.request()?,
            InputEnvelope::Item(_) => self.item()?,
        }
        Ok(envelope)
    }

    pub fn request(&mut self) -> Result<(), Status> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.opened {
            let error = Status::invalid_argument("duplicate opening request");
            self.fail(error.clone());
            return Err(error);
        }
        self.opened = true;
        Ok(())
    }

    pub fn item(&mut self) -> Result<(), Status> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if !self.opened {
            let error = Status::invalid_argument("input item before opening request");
            self.fail(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub fn fail(&mut self, error: Status) {
        if self.failure.is_none() {
            self.failure = Some(error);
        }
    }

    pub fn finish(self, termination: Result<(), Status>) -> Result<(), Status> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        termination?;
        if !self.opened {
            return Err(Status::invalid_argument("missing opening request"));
        }
        Ok(())
    }
}

/* === Public Functions === */

pub fn stream<O: crate::operations::OutputDeclaration>(
    instance: crate::operations::OperationInstance<O>,
    items: crate::operations::StreamInput<O::Item>,
) -> ResponseStream<O, O::Item> {
    ResponseStream {
        pending: Box::pin(response(instance)),
        items: Some(items.into_receiver()),
        result: None,
    }
}

pub fn exchange<O, S>(
    instance: crate::operations::OperationInstance<O>,
    input: crate::operations::StreamOutput<<O as crate::operations::InputDeclaration>::Item>,
    incoming: S,
    output: crate::operations::StreamInput<<O as crate::operations::OutputDeclaration>::Item>,
) -> ResponseStream<O, <O as crate::operations::OutputDeclaration>::Item, InputError>
where
    O: crate::operations::InputDeclaration + crate::operations::OutputDeclaration,
    <O as crate::operations::InputDeclaration>::Item: crate::MemorySize,
    S: Stream<
            Item = Result<
                crate::operations::StreamItem<<O as crate::operations::InputDeclaration>::Item>,
                tonic::Status,
            >,
        > + Send
        + 'static,
{
    ResponseStream {
        pending: Box::pin(self::input(instance, input, incoming)),
        items: Some(output.into_receiver()),
        result: None,
    }
}

pub fn response<O: crate::operations::OperationDeclaration>(
    instance: crate::operations::OperationInstance<O>,
) -> impl std::future::Future<Output = crate::Result<std::sync::Arc<O::Response>>> + Send {
    let pending = Pending {
        instance,
        delivered: false,
    };
    async move {
        let mut pending = pending;
        let result = (&mut pending.instance).await;
        pending.delivered = true;
        result
    }
}

/* === Private Functions === */

/* === Tests === */
