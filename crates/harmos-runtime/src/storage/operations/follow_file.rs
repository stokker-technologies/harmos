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

use harmos_domain::operations::storage::FollowFile;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Operation, OperationDeclaration, StreamOutput};
use harmos_domain::model::storage::files::{FileCompletion, FileEvent};

use crate::storage::StorageManager;

use super::IO_TIMEOUT;
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
    type Request = <FollowFile as harmos_domain::OperationDeclaration>::Request;
    type Response = <FollowFile as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<FileEvent>;
    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_file", level = "trace", skip_all)]
    async fn run(
        &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileCompletion>> {
        let host = ctx.dependency::<crate::operations::HostContext>()?;
        let ctx = host.borrow();
        let grant = request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(FollowFile::ID, scope));
        if let Some(grant) = &grant {
            ctx.admit_stream_grant(grant)?;
        }
        let mut follower = self.storage.follow_file(request.clone()).await?;
        while let Some(event) = follower.next().await? {
            if let Some(grant) = &grant {
                ctx.admit_stream_grant(grant)?;
            }
            let response = crate::operations::Response::from_owner(event.owner());
            tokio::time::timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| super::output_timeout())??;
        }
        Ok(follower.completion()?.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
