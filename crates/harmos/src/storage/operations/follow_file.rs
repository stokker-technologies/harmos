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
use std::any::TypeId;

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;
use crate::storage::follow::File;
use crate::storage::{FileBatch, FileProgress};

use crate::storage::StorageManager;

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowFile {
    type Request = File;
    type Response = FileProgress;
    const ID: &'static str = "storage.follow_file";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBatch>()))
    }
}
impl OutputDeclaration for FollowFile {
    type Item = FileBatch;
}
impl WireOperation for FollowFile {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = FollowFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileBatch>;
    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut File,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProgress>> {
        let mut follower = self
            .storage
            .follow_file(&request.file, request.offset)
            .await?;
        let batch_bytes = self.storage.inner.settings.response.batch_bytes;
        while let Some(batch) = follower.next_batch(batch_bytes).await? {
            output.send_with(batch.memory_size(), || Ok(batch)).await?;
        }
        Ok(follower.progress().into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
