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
use super::{CallContext, OperationInstance, OperationManager, StreamItem};
use crate::Result;
use crate::operations::OperationDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct GrpcBinding {
    pub(super) operations: OperationManager,
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

    pub(crate) fn submit<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: StreamItem<O::Request>,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.operations.admit_entry::<O, P>(
            (super::binding::Request::charged(request), endpoints),
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
