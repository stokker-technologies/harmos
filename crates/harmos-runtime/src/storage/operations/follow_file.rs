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

use crate::operations::{Context, Operation, StreamOutput};
use crate::storage::{FileBatch, FileProgress};

use crate::storage::StorageManager;

use crate::{Error, MemorySize, Result};

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
    type Declaration = FollowFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileBatch>;
    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut FollowFile, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProgress>> {
        let mut follower = self.storage.follow_file(&request.file, request.offset).await?;
        let batch_bytes = self.storage.settings().response.batch_bytes;
        while let Some(batch) = follower.next_batch(batch_bytes).await? {
            output.send_with(batch.memory_size(), || Ok(batch)).await?;
        }
        Ok(follower.progress().into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
