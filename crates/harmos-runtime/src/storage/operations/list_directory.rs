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
use crate::operations::{Context, Operation, Response, StreamOutput};
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
    type Declaration = ListDirectory;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<DirectoryEntry>;

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.list_directory", level = "trace", skip_all)]
    async fn run<'a>(
        &mut self, _ctx: Self::Context<'a>, request: &mut ListDirectory, output: &mut Self::Endpoints,
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
