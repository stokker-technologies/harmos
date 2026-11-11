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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
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
        let submitted = self.clock().now();
        let entry = self.capture_entry(O::ID)?;
        let source = Source::Host;
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
        let payload = request.encode_to_vec();
        let mut preparation = self.claim_preparation(NonZeroUsize::new(payload.capacity().max(1)).unwrap())?;
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
