/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finite Storage Query Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{Operation, Response, StreamOutput};
use crate::storage::StorageManager;
use crate::{Error, Result};
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEvent};
use harmos_domain::operations::storage::WatchDirectory;

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
    type Request = <WatchDirectory as harmos_domain::OperationDeclaration>::Request;
    type Response = <WatchDirectory as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<DirectoryEvent>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.watch_directory", level = "trace", skip_all)]
    async fn run(
        &mut self, _ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<DirectoryCompletion>> {
        let mut reader = self.storage.watch_directory(request.selection.clone(), request.include_existing).await?;
        let mut entries = 0_u64;
        while let Some(item) = reader.next().await? {
            if matches!(&*item, DirectoryEvent::Existing(_) | DirectoryEvent::Created(_) | DirectoryEvent::Modified(_)) {
                entries = entries.checked_add(1).ok_or(Error::ResourceExhausted { resource: "directory observation count" })?;
            }
            let response = Response::from_owner(item.owner());
            tokio::time::timeout(super::IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| super::output_timeout())??;
        }
        Ok(DirectoryCompletion { entries }.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
