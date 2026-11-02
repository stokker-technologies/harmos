/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Remote Operation Calls

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{future::Future, marker::PhantomData, pin::Pin};

// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status, Streaming, body::Body, codegen::http};

// Local Imports
use super::{Codec, Event, Wire, server};
use crate::operations::{Direction, OperationDeclaration, StreamItem};
use crate::{MemorySize, MessageError};
use harmos_sdk::grpc::DecodeInput;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<O> = Event<
    StreamItem<<<O as OperationDeclaration>::Output as Direction>::Item>,
    crate::operations::Response<<O as OperationDeclaration>::Response>,
>;
type Events<O> = Pin<Box<dyn Stream<Item = Result<Output<O>, Status>> + Send>>;
type Forward = Pin<Box<dyn Future<Output = Result<bool, Status>> + Send>>;

/* === Structs === */

struct Endpoint<O> {
    authority: server::Authority,
    settings: server::Settings,
    operation: PhantomData<fn() -> O>,
}

/* === Enums === */

/// Frames Of A Managed Call; Cancellation Keeps The Response Stream Open.
#[derive(Debug, PartialEq)]
pub enum ControlEnvelope<R, I> {
    Request(R),
    Item(I),
    Cancel,
    Finish,
}

/* === Traits === */

/* === Implementations === */

impl<R: MemorySize, I: MemorySize> MemorySize for ControlEnvelope<R, I> {
    fn memory_size(&self) -> usize {
        let heap = match self {
            Self::Request(value) => value.memory_size().saturating_sub(std::mem::size_of::<R>()),
            Self::Item(value) => value.memory_size().saturating_sub(std::mem::size_of::<I>()),
            Self::Cancel | Self::Finish => 0,
        };
        std::mem::size_of::<Self>().saturating_add(heap)
    }
}

impl<W, R, I> Wire<ControlEnvelope<R, I>> for W
where
    W: Wire<R> + Wire<I>,
    R: Send + Sync + 'static,
    I: Send + Sync + 'static,
{
    fn encode(value: &ControlEnvelope<R, I>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        match value {
            ControlEnvelope::Request(value) => super::envelope::delimited(1, bytes, |bytes| <W as Wire<R>>::encode(value, bytes)),
            ControlEnvelope::Item(value) => super::envelope::delimited(2, bytes, |bytes| <W as Wire<I>>::encode(value, bytes)),
            ControlEnvelope::Cancel => super::envelope::delimited(3, bytes, |_| Ok(())),
            ControlEnvelope::Finish => super::envelope::delimited(4, bytes, |_| Ok(())),
        }
    }

    fn encode_admitted(
        value: &ControlEnvelope<R, I>, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
    ) -> Result<(), Status> {
        match value {
            ControlEnvelope::Request(value) => super::envelope::delimited_admitted(1, bytes, limit, admit, |bytes, admit| {
                <W as Wire<R>>::encode_admitted(value, bytes, limit, admit)
            }),
            ControlEnvelope::Item(value) => super::envelope::delimited_admitted(2, bytes, limit, admit, |bytes, admit| {
                <W as Wire<I>>::encode_admitted(value, bytes, limit, admit)
            }),
            ControlEnvelope::Cancel => super::envelope::delimited_admitted(3, bytes, limit, admit, |_, _| Ok(())),
            ControlEnvelope::Finish => super::envelope::delimited_admitted(4, bytes, limit, admit, |_, _| Ok(())),
        }
    }

    fn decode_admitted<C>(
        input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, Status>,
    ) -> Result<(ControlEnvelope<R, I>, C), Status> {
        let (tag, payload) = input
            .variant(&[1, 2, 3, 4])
            .map_err(|error| Status::invalid_argument(error.to_string()))?
            .ok_or_else(|| Status::invalid_argument("empty managed envelope"))?;
        let admit = |bound: usize| admit(bound.saturating_add(std::mem::size_of::<ControlEnvelope<R, I>>()));
        match tag {
            1 => <W as Wire<R>>::decode_admitted(payload, admit).map(|(value, claim)| (ControlEnvelope::Request(value), claim)),
            2 => <W as Wire<I>>::decode_admitted(payload, admit).map(|(value, claim)| (ControlEnvelope::Item(value), claim)),
            _ => {
                if !payload.contiguous().map_err(|error| Status::invalid_argument(error.to_string()))?.is_empty() {
                    return Err(Status::invalid_argument("cancellation payload must be empty"));
                }
                Ok((if tag == 3 { ControlEnvelope::Cancel } else { ControlEnvelope::Finish }, admit(0)?))
            }
        }
    }
}

impl<O: OperationDeclaration> tonic::server::StreamingService<StreamItem<ControlEnvelope<O::Request, <O::Input as Direction>::Item>>>
    for Endpoint<O>
{
    type Response = Output<O>;
    type ResponseStream = Events<O>;
    type Future = tonic::codegen::BoxFuture<Response<Self::ResponseStream>, Status>;

    fn call(
        &mut self, request: Request<Streaming<StreamItem<ControlEnvelope<O::Request, <O::Input as Direction>::Item>>>>,
    ) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let mut incoming = request.into_inner();
            let first = tokio::time::timeout(settings.opening_timeout, incoming.next())
                .await
                .map_err(|_| Status::deadline_exceeded("operation opening timed out"))?
                .transpose()?
                .ok_or_else(|| Status::invalid_argument("missing opening request"))?;
            let request = first
                .project(|frame| match frame {
                    ControlEnvelope::Request(request) => Ok(request),
                    _ => Err(()),
                })
                .map_err(|error| server::status(error.into()))?
                .map_err(|_| Status::invalid_argument("opening request required"))?;
            let (input, receiving) = crate::operations::stream::<<O::Input as Direction>::Item>(settings.input).map_err(server::status)?;
            let (sending, items) = crate::operations::stream::<<O::Output as Direction>::Item>(settings.output).map_err(server::status)?;
            let instance = match (<O::Input as Direction>::STREAMED, <O::Output as Direction>::STREAMED) {
                (false, false) => authority.submit_charged::<O, _>(request, ()),
                (true, false) => authority.submit_charged::<O, _>(request, receiving),
                (false, true) => authority.submit_charged::<O, _>(request, sending),
                (true, true) => authority.submit_charged::<O, _>(request, (receiving, sending)),
            }
            .map_err(server::status)?;
            let control = instance.control.clone();
            let forward: Forward = Box::pin(async move {
                let mut input = Some(input);
                while let Some(frame) = incoming.next().await {
                    let frame = frame?;
                    if matches!(&*frame, ControlEnvelope::Cancel) {
                        return Ok(true);
                    }
                    if !<O::Input as Direction>::STREAMED {
                        return Err(Status::invalid_argument("operation has no input stream"));
                    }
                    if matches!(&*frame, ControlEnvelope::Finish) {
                        if input.take().is_none() {
                            return Err(Status::invalid_argument("input already finished"));
                        }
                        continue;
                    }
                    let input = input.as_ref().ok_or_else(|| Status::invalid_argument("input already finished"))?;
                    let item = frame
                        .project(|frame| match frame {
                            ControlEnvelope::Item(item) => Ok(item),
                            _ => Err(()),
                        })
                        .map_err(|error| server::status(error.into()))?
                        .map_err(|_| Status::invalid_argument("duplicate opening request"))?;
                    input.send_item(item).await.map_err(|error| server::status(error.into()))?;
                }
                Ok(false)
            });
            let events: Events<O> = Box::pin(super::stream(instance, items).map(|answer| answer.map_err(server::status)));
            let state = (events, Some(forward), control);
            let output = futures_util::stream::unfold(Some(state), |state| async move {
                let (mut events, mut forward, control) = state?;
                loop {
                    tokio::select! {
                        biased;
                        event = events.next() => return event.map(|event| (event, Some((events, forward, control)))),
                        result = async { forward.as_mut().expect("enabled forwarding").await }, if forward.is_some() => {
                            forward = None;
                            match result {
                                Ok(true) => control.request_cancel(),
                                Ok(false) => {},
                                Err(error) => { control.request_cancel(); return Some((Err(error), None)); }
                            }
                        }
                    }
                }
            });
            Ok(Response::new(Box::pin(output) as Events<O>))
        })
    }
}

/* === Public Functions === */

pub(super) fn is_managed(authority: &server::Authority, path: &str) -> bool {
    match authority {
        server::Authority::Binding(binding) => binding.contract().grpc().is_some_and(|grpc| path == grpc.run_path()),
        _ => false,
    }
}

pub(super) fn serve<O, W>(authority: server::Authority, settings: server::Settings, request: http::Request<Body>) -> server::HttpFuture
where
    O: OperationDeclaration,
    W: Wire<O::Request> + Wire<<O::Input as Direction>::Item> + Wire<<O::Output as Direction>::Item> + Wire<O::Response>,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<W, ControlEnvelope<O::Request, <O::Input as Direction>::Item>, Output<O>>::new(
            settings.decode,
            settings.encode,
        ) {
            Ok(codec) => codec.with_preparation(authority.preparation()),
            Err(error) => return error.into_http(),
        };
        codec
            .server()
            .streaming(
                Endpoint::<O> {
                    authority,
                    settings,
                    operation: PhantomData,
                },
                request,
            )
            .await
    })
}

/* === Private Functions === */

/* === Tests === */
