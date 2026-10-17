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
use harmos_domain::operations::storage::ReadFile;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use super::IO_TIMEOUT;
use crate::operations::{Context, Operation, OperationDeclaration, StreamOutput};
use crate::storage::StorageManager;
use crate::{Error, MemorySize, Result};
use harmos_domain::model::storage::files::{FileBatch, FileCompletion};

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
    type Declaration = ReadFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileBatch>;

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, ctx: Self::Context<'a>, request: &mut ReadFile, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileCompletion>> {
        let grant = harmos_domain::plugin::Grant::of(ReadFile::ID, request.file.metadata.id.as_str().to_owned());
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self.storage.read_file(request.clone()).await?;
        while let Some(batch) = reader.next().await? {
            ctx.admit_stream_grant(&grant)?;
            let charge = batch.memory_size();
            tokio::time::timeout(IO_TIMEOUT, output.send_with(charge, || Ok(batch))).await.map_err(|_| super::output_timeout())??;
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
