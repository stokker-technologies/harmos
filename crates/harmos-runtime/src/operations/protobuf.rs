/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Invoke Generated Host Declarations Through Native Admission.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::caller::Source;
use super::manager::Submission;
use super::{EncodedInvocation, Manager, ProtobufOperation, WireStreamLimits, WireStreams};
use crate::{Error, Result};
use prost::Message;
use std::{num::NonZeroUsize, sync::Arc};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ProtobufDownload<O: ProtobufOperation> {
    invocation: EncodedInvocation,
    streams: WireStreams,
    declaration: std::marker::PhantomData<fn() -> O>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<O: ProtobufOperation> ProtobufDownload<O>
where
    O::Endpoints: harmos_operations::execution::operations::OutputEndpoint,
    <O::Endpoints as harmos_operations::execution::operations::OutputEndpoint>::Item: Message + Default,
{
    pub async fn next(&mut self) -> Result<Option<<O::Endpoints as harmos_operations::execution::operations::OutputEndpoint>::Item>> {
        self.streams
            .receive_current()
            .await
            .map_err(|source| Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            })?
            .map(|bytes| {
                Message::decode(bytes.as_slice()).map_err(|error: prost::DecodeError| Error::OperationPayload {
                    operation: O::ID.into(),
                    source: crate::MessageError::new(error.to_string()),
                })
            })
            .transpose()
    }

    pub fn request_cancel(&self) {
        self.invocation.request_cancel();
    }

    pub async fn join(self) -> Result<O::Response> {
        O::Response::decode(self.invocation.await?.as_slice()).map_err(|error| Error::OperationPayload {
            operation: O::ID.into(),
            source: crate::MessageError::new(error.to_string()),
        })
    }
}

impl Manager {
    pub fn download_protobuf<O: ProtobufOperation>(&self, request: O::Request, output: super::StreamSettings) -> Result<ProtobufDownload<O>>
    where
        O::Endpoints: harmos_operations::execution::operations::OutputEndpoint,
        <O::Endpoints as harmos_operations::execution::operations::OutputEndpoint>::Item: Message + Default,
    {
        let (invocation, streams) = self.submit_protobuf::<O>(request, Some(WireStreamLimits { input: output, output }))?;
        Ok(ProtobufDownload {
            invocation,
            streams: streams.ok_or(Error::InvalidInput {
                field: "protobuf download",
                reason: "output endpoint required",
            })?,
            declaration: std::marker::PhantomData,
        })
    }

    pub async fn call_protobuf<O: ProtobufOperation<Endpoints = ()>>(&self, request: O::Request) -> Result<O::Response> {
        let (invocation, _) = self.submit_protobuf::<O>(request, None)?;
        let bytes = invocation.await?;
        O::Response::decode(bytes.as_slice()).map_err(|error| Error::OperationPayload {
            operation: O::ID.into(),
            source: crate::MessageError::new(error.to_string()),
        })
    }

    pub fn submit_protobuf<O: ProtobufOperation>(
        &self, request: O::Request, limits: Option<WireStreamLimits>,
    ) -> Result<(EncodedInvocation, Option<WireStreams>)> {
        self.submit_protobuf_as::<O>(request, limits, Source::Host)
    }

    pub(super) fn submit_protobuf_as<O: ProtobufOperation>(
        &self, request: O::Request, limits: Option<WireStreamLimits>, source: Source,
    ) -> Result<(EncodedInvocation, Option<WireStreams>)> {
        let submitted = self.clock().now();
        let entry = self.capture_entry(O::ID)?;
        let origin = source.check()?;
        entry.available_to(&origin)?;
        entry.authorize(&origin)?;
        let factory =
            entry.binding.downcast_ref::<harmos_operations::execution::operations::OperationFactory>().ok_or(Error::InvalidInput {
                field: "protobuf operation",
                reason: "native declaration required",
            })?;
        let expected =
            harmos_operations::execution::operations::OperationFactory::protobuf::<O>().map_err(|source| Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            })?;
        if expected.definition.fingerprint != entry.declared().fingerprint {
            return Err(Error::InvalidInput {
                field: "protobuf operation",
                reason: "declaration differs from registered contract",
            });
        }
        let mut preparation = self.claim_preparation(NonZeroUsize::new(request.encoded_len().max(1)).unwrap())?;
        let payload = request.encode_to_vec();
        let memory =
            factory.grpc_decoding_memory(&payload).map_err(|source| Error::OperationPayload { operation: O::ID.into(), source })?;
        if let Some(memory) = NonZeroUsize::new(memory) {
            preparation.merge(self.claim_preparation(memory)?);
        }
        let prepared =
            factory.prepare_envelope(self.resources_for(&entry), &payload, true, limits).map_err(|source| Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            })?;
        let streams = prepared.streams.clone();
        let invocation = super::wire::dispatch_prepared(
            self,
            Submission {
                submitted,
                entry,
                cause: None,
                preparation: Some(preparation),
            },
            source,
            None,
            prepared,
            super::binding::Request::from(payload),
        )?;
        Ok((invocation, streams))
    }
}

#[cfg(feature = "plugin-guest")]
impl super::Caller {
    pub fn download_protobuf<O: ProtobufOperation>(&self, request: O::Request, output: super::StreamSettings) -> Result<ProtobufDownload<O>>
    where
        O::Endpoints: harmos_operations::execution::operations::OutputEndpoint,
        <O::Endpoints as harmos_operations::execution::operations::OutputEndpoint>::Item: Message + Default,
    {
        let (invocation, streams) = self.operations.submit_protobuf_as::<O>(
            request,
            Some(WireStreamLimits { input: output, output }),
            Source::Plugin(self.source.clone()),
        )?;
        Ok(ProtobufDownload {
            invocation,
            streams: streams.ok_or(Error::InvalidInput {
                field: "protobuf download",
                reason: "output endpoint required",
            })?,
            declaration: std::marker::PhantomData,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
