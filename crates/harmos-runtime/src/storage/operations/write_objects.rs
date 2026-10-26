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

use harmos_domain::operations::storage::WriteObjects;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{Operation, OperationDeclaration, StreamInput, StreamOutput};
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
    type Request = <WriteObjects as harmos_domain::OperationDeclaration>::Request;
    type Response = <WriteObjects as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = (StreamInput<DataBatch>, StreamOutput<WriteReceipt>);

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<WriteCompletion>> {
        let host = ctx.dependency::<crate::operations::HostContext>()?;
        let ctx = host.borrow();
        let completion = exchange(&self.storage, &ctx, request.scope.as_ref(), endpoints, WriteKind::Objects, WriteObjects::ID).await?;
        Ok(completion.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
