/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Managed Remote Operation Handles

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{borrow::Borrow, sync::Arc};

// Third Party Imports
use tonic::{Request, Status, Streaming, transport::Channel};

// Local Imports
use super::{Codec, Completion, ControlEnvelope, Encode, OutputEnvelope, Transcoded, Wire};
use crate::operations::{Direction, OperationDeclaration, RequestClaims, Response, StreamItem, StreamOutput};
use crate::{MemorySize, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Input<O> = ControlEnvelope<<O as OperationDeclaration>::Request, <<O as OperationDeclaration>::Input as Direction>::Item>;
type Output<O> = OutputEnvelope<<<O as OperationDeclaration>::Output as Direction>::Item, <O as OperationDeclaration>::Response>;

/* === Structs === */

#[derive(Clone, Copy)]
pub struct Settings {
    pub decode: super::DecodeSettings,
    pub encode: super::EncodeSettings,
    pub input: crate::operations::StreamSettings,
}

/// One Managed Remote Call. Cancel Signals The Server And Awaits Its Typed Response.
#[must_use = "dropping a remote operation abandons its response and requests cancellation"]
pub struct Running<O: OperationDeclaration> {
    outgoing: StreamOutput<Input<O>>,
    input_finished: bool,
    incoming: Streaming<StreamItem<Output<O>>>,
    completion: Option<Completion<Response<O::Response>>>,
}

struct Frame<T>(StreamItem<T>);
struct Received<T> {
    value: T,
    _claims: RequestClaims,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T> Borrow<T> for Received<T> {
    fn borrow(&self) -> &T {
        &self.value
    }
}

impl<W: Wire<T>, T: Send + Sync + 'static> Encode<W> for Frame<T> {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        W::encode(&self.0, bytes)
    }
    fn write_admitted(&self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
        W::encode_admitted(&self.0, bytes, limit, admit)
    }
}

impl<O: OperationDeclaration> Running<O> {
    pub async fn request_cancel(&self) -> Result<(), Status> {
        self.send_frame(ControlEnvelope::Cancel).await
    }

    pub async fn cancel(self) -> Result<Response<O::Response>, Status> {
        // A Completed Server May Already Have Closed Its Input.
        let _ = self.request_cancel().await;
        self.join().await
    }

    pub async fn send(&self, item: <O::Input as Direction>::Item) -> Result<(), Status> {
        if self.input_finished || !<O::Input as Direction>::STREAMED {
            return Err(Status::invalid_argument("operation has no input stream"));
        }
        self.send_frame(ControlEnvelope::Item(item)).await
    }

    pub async fn finish_input(&mut self) -> Result<(), Status> {
        if self.input_finished || !<O::Input as Direction>::STREAMED {
            return Err(Status::failed_precondition("operation input is closed"));
        }
        self.send_frame(ControlEnvelope::Finish).await?;
        self.input_finished = true;
        Ok(())
    }

    pub async fn next(&mut self) -> Result<Option<StreamItem<<O::Output as Direction>::Item>>, Status> {
        let result = self.receive().await;
        if let Err(error) = &result
            && let Some(completion) = &mut self.completion
        {
            completion.fail(error.clone());
        }
        result
    }

    async fn receive(&mut self) -> Result<Option<StreamItem<<O::Output as Direction>::Item>>, Status> {
        while let Some(frame) = self.incoming.message().await? {
            let frame = frame
                .project(|frame| match frame {
                    OutputEnvelope::Item(item) => Ok(item),
                    OutputEnvelope::Response(response) => Err(response),
                })
                .map_err(|error| super::server::status(error.into()))?;
            let completion = self.completion.as_mut().ok_or_else(|| Status::failed_precondition("remote operation already joined"))?;
            match frame {
                Ok(item) => {
                    completion.item()?;
                    return Ok(Some(item));
                }
                Err(response) => {
                    let (value, claims) = response.into_request().map_err(|error| super::server::status(error.into()))?;
                    completion.complete(Response::from_owner(Arc::new(Received { value, _claims: claims })))?;
                }
            }
        }
        Ok(None)
    }

    pub async fn join(mut self) -> Result<Response<O::Response>, Status> {
        while self.next().await?.is_some() {}
        self.completion.take().expect("unjoined operation").finish(Ok(()))
    }

    async fn send_frame(&self, frame: Input<O>) -> Result<(), Status> {
        let bytes = frame.memory_size();
        self.outgoing.send_with(bytes, || Ok(frame)).await.map_err(|error| super::server::status(error.into()))
    }
}

/* === Public Functions === */

/// Starts A Managed Call With Consumer-Supplied Transport And Buffer Limits.
pub async fn start<O: OperationDeclaration>(
    channel: Channel, request: Request<O::Request>, settings: Settings,
) -> Result<Running<O>, Status> {
    settings.decode.validate()?;
    settings.encode.validate()?;
    let grpc = O::GRPC.ok_or_else(|| Status::invalid_argument("operation has no gRPC declaration"))?;
    let path = grpc.run_path().parse().map_err(|_| Status::invalid_argument("invalid managed path"))?;
    let (outgoing, incoming) = crate::operations::stream::<Input<O>>(settings.input).map_err(super::server::status)?;
    let (metadata, extensions, request) = request.into_parts();
    let frame = ControlEnvelope::Request(request);
    outgoing.send_with(frame.memory_size(), || Ok(frame)).await.map_err(|error| super::server::status(error.into()))?;
    let stream = futures_util::stream::unfold(incoming.into_receiver(), |mut receiver| async move {
        receiver.recv().await.map(|frame| (Frame(frame), receiver))
    });
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|error| Status::unavailable(error.to_string()))?;
    let codec = Codec::<Transcoded, Output<O>, Frame<Input<O>>>::new(settings.decode, settings.encode)?;
    let incoming = client.streaming(Request::from_parts(metadata, extensions, stream), path, codec).await?.into_inner();
    Ok(Running {
        outgoing,
        input_finished: false,
        incoming,
        completion: Some(Completion::default()),
    })
}

/* === Private Functions === */

/* === Tests === */
