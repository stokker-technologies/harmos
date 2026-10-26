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
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry};
use harmos_domain::operations::storage::ListDirectory;

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
    type Request = <ListDirectory as harmos_domain::OperationDeclaration>::Request;
    type Response = <ListDirectory as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<DirectoryEntry>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.list_directory", level = "trace", skip_all)]
    async fn run(
        &mut self, _ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<DirectoryCompletion>> {
        let mut reader = self.storage.list_directory(request.selection.clone()).await?;
        while let Some(item) = reader.next().await? {
            let response = Response::from_owner(item.owner());
            tokio::time::timeout(super::IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| super::output_timeout())??;
        }
        Ok(reader.completion()?.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
