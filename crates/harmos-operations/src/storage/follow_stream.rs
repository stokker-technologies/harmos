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

// Third Party Imports

// Local Imports
use super::STREAM_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, Optional, product};
use harmos_domain::model::storage::{StorageCursor, StoredRow, StreamSelection};
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(FollowStream {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<StreamSelection> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.follow_stream",
    response = StorageCursor,
    policy = STREAM_POLICY,
    output = StoredRow,
    visibility = Host)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowStream {
    pub after: Option<StorageCursor>,
    pub selection: StreamSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowStream {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::{StorageCursor, StoredRow};

    use crate::{Error, MemorySize, Result};

    impl Operation for super::FollowStream {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StorageCursor>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let output = context.streams();

            harmos_storage::query::Source::Stream(self.selection.clone()).validate()?;
            let mut follower = storage.follow::<StoredRow>(self.after.as_ref(), self.selection.clone()).await?;
            loop {
                let page = follower.next().await?;
                for entry in page.entries() {
                    output.send_with(entry.memory_size(), || Ok(entry.clone())).await?;
                }
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
