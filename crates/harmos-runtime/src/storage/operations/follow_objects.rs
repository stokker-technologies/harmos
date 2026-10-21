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

use harmos_domain::operations::storage::FollowObjects;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation, StreamOutput};

use crate::storage::StorageManager;
use crate::storage::{ObjectEntry, StorageCursor};

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
    type Declaration = FollowObjects;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<ObjectEntry>;

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut FollowObjects, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        if !request.selection.follows_revisions() {
            return Err(Error::InvalidInput { field: "storage follow", reason: "following delivers every object revision" });
        }
        let mut follower = self.storage.follow::<ObjectEntry>(request.after.as_ref(), request.selection.clone()).await?;
        loop {
            let page = follower.next().await?;
            for entry in page.entries() {
                output.send_with(entry.memory_size(), || Ok(entry.clone())).await?;
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
