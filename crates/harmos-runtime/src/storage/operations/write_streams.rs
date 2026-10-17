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
use harmos_domain::operations::storage::WriteStreams;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{Context, Operation, OperationDeclaration, StreamInput, StreamOutput};
use crate::storage::StorageManager;
use crate::storage::writes::{DataBatch, WriteCompletion, WriteKind, WriteReceipt, exchange};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteStreams;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = (StreamInput<DataBatch>, StreamOutput<WriteReceipt>);

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, ctx: Self::Context<'a>, request: &mut WriteStreams, endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<WriteCompletion>> {
        let completion = exchange(&self.storage, &ctx, request.scope.as_ref(), endpoints, WriteKind::Streams, WriteStreams::ID).await?;
        Ok(completion.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
