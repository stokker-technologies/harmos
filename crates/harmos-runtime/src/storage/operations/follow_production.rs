/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Follow An Existing Staged Production Without Reinterpreting Its Identity.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::IO_TIMEOUT;
use crate::operations::{Operation, OperationDeclaration, StreamOutput};
use crate::storage::{FileBatch, FileProgress, StorageManager};
use crate::{Error, Result};
use harmos_domain::operations::storage::FollowProduction;

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
    type Request = <FollowProduction as harmos_domain::OperationDeclaration>::Request;
    type Response = <FollowProduction as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<FileBatch>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_production", level = "trace", skip_all)]
    async fn run(
        &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProgress>> {
        let host = ctx.dependency::<crate::operations::HostContext>()?;
        let ctx = host.borrow();
        let grant = harmos_domain::plugin::Grant::of(FollowProduction::ID, request.file.id.as_str().to_owned());
        ctx.admit_stream_grant(&grant)?;
        let mut follower = self.storage.follow_production(&request.file, request.offset).await?;
        let batch_bytes = self.storage.settings().response.batch_bytes;
        while let Some(batch) = follower.next_batch(batch_bytes).await? {
            ctx.admit_stream_grant(&grant)?;
            let response = crate::operations::Response::from_owner(batch.owner());
            tokio::time::timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| super::output_timeout())??;
        }
        Ok(follower.progress().into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
