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
use harmos_domain::operations::storage::{FollowBoundaries, FollowRecords, Followed};

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation, StreamOutput};

use crate::storage::StorageManager;
use crate::storage::{StorageCursor, StoredRecord};

use crate::MemorySize;
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
    type Declaration = FollowRecords;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<Followed>;

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut FollowRecords, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        crate::storage::query::Source::Records(request.selection.clone()).validate()?;
        let mut follower = self.storage.follow::<StoredRecord>(request.after.as_ref(), request.selection.clone()).await?;
        let first = Followed::Boundaries(FollowBoundaries {
            saved: follower.saved_boundary().clone(),
            accepted: follower.accepted_boundary().clone(),
        });
        output.send_with(first.memory_size(), || Ok(first.clone())).await?;
        loop {
            let page = follower.next().await?;
            for entry in page.entries() {
                let wrapped = Followed::wrapping_memory(entry.memory_size());
                output.send_with(wrapped, || Ok(Followed::Record(entry.clone()))).await?;
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
