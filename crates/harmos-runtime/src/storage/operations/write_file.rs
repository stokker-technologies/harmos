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

use harmos_domain::operations::storage::WriteFile;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{Operation, OperationDeclaration, StreamInput};
use crate::storage::{FileReference, StorageManager};
use crate::{Error, Result};
use harmos_domain::model::storage::files::FileBatch;

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
    type Request = <WriteFile as harmos_domain::OperationDeclaration>::Request;
    type Response = <WriteFile as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamInput<FileBatch>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, input: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileReference>> {
        let host = ctx.dependency::<crate::operations::HostContext>()?;
        let ctx = host.borrow();
        let grant = harmos_domain::plugin::Grant::of(WriteFile::ID, request.metadata.id.as_str().to_owned());
        ctx.admit_stream_grant(&grant)?;
        self.storage.retry_transient_persistence();
        let mut file = self.storage.create_file(request.metadata.clone()).await?;

        let mut offset = 0u64;
        while let Some(batch) = input.recv().await {
            let (batch, _claims) = batch.into_owned_request()?;
            ctx.admit_stream_grant(&grant)?;
            if batch.offset != offset || batch.bytes.is_empty() {
                return Err(Error::InvalidInput { field: "file batch", reason: "nonempty batches must have contiguous byte offsets" });
            }
            let next = offset
                .checked_add(batch.bytes.len() as u64)
                .ok_or(Error::InvalidInput { field: "file batch offset", reason: "byte offset overflow" })?;
            file.write_all(batch.bytes).await?;
            offset = next;
        }

        ctx.admit_stream_grant(&grant)?;
        let reference = file.finish().await?;
        self.storage.await_durable(&reference.position).await?;
        Ok(reference.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
