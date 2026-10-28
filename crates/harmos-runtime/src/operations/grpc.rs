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

// Local Imports
use super::caller::Source;
use super::manager::{Entry, Submission};
use super::{CallContext, Manager, Running, StreamItem};
use crate::Result;
use crate::operations::OperationDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct GrpcBinding {
    pub(crate) operations: Manager,
    pub(super) entry: Arc<Entry>,
}

#[derive(Clone, Debug)]
pub struct GrpcMethod {
    pub owner: crate::Identifier,
    pub generation: super::OperationGeneration,
    pub contract: crate::operations::Contract,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl GrpcBinding {
    pub(crate) fn with_context(&self, context: CallContext) -> Self {
        Self {
            operations: self.operations.with_context(context),
            entry: self.entry.clone(),
        }
    }

    pub(crate) fn handler(&self) -> Option<crate::grpc::routing::Handler> {
        self.entry.grpc
    }

    pub(crate) fn contract(&self) -> &crate::operations::Contract {
        self.entry.contract.as_ref().expect("gRPC binding owns a contract")
    }

    pub(crate) fn native_factory(&self) -> Result<harmos_sdk::operations::OperationFactory> {
        self.entry.binding.downcast_ref::<harmos_sdk::operations::OperationFactory>().cloned().ok_or(crate::Error::InvalidInput {
            field: "gRPC native binding",
            reason: "native factory required",
        })
    }

    pub(crate) fn submit_native(
        &self, payload: StreamItem<Vec<u8>>, limits: harmos_sdk::operations::WireStreamLimits,
    ) -> Result<(super::EncodedInvocation, Option<harmos_sdk::operations::WireStreams>)> {
        let source = Source::Host.external()?;
        let origin = source.check()?;
        self.entry.available_to(&origin)?;
        self.entry.authorize(&origin)?;
        let factory = self.native_factory()?;
        let name = self.entry.id().to_owned();
        let resources = self.operations.resources_for(&self.entry);
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| factory.prepare_grpc(resources, &payload, limits)))
            .map_err(|_| crate::Error::OperationPanicked { operation: name.clone() })?
            .map_err(|source| crate::Error::OperationFailed {
                operation: name,
                source: Arc::new(source),
            })?;
        let streams = prepared.streams.clone();
        let invocation = super::wire::dispatch_prepared(
            &self.operations,
            Submission {
                submitted: crate::supervision::executor::now(),
                entry: self.entry.clone(),
                cause: None,
                preparation: None,
            },
            source,
            None,
            prepared,
            super::binding::Request::charged(payload)?,
        )?;
        Ok((invocation, streams))
    }

    pub(crate) fn submit<O: OperationDeclaration, P: Send + 'static>(
        &self, request: StreamItem<O::Request>, endpoints: P,
    ) -> Result<Running<O>> {
        self.operations.admit_entry::<O, P>(
            (super::binding::Request::charged(request)?, endpoints),
            None,
            &[],
            &Source::Host.external()?,
            Submission {
                submitted: crate::supervision::executor::now(),
                entry: self.entry.clone(),
                cause: None,
                preparation: None,
            },
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
