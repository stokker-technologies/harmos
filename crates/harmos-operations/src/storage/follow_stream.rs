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
use harmos_domain::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowStream {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<StreamSelection> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [harmos_storage::StorageManager<harmos_storage::ErasedCatalog>], codec = manual,
    id = "storage.follow_stream",
    response = StorageCursor,
    policy = STREAM_POLICY,
    output = StoredRow,
    visibility = Visibility::Host)]
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

        async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StorageCursor>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let request = &mut self;
            let output = context.streams();

            harmos_storage::query::Source::Stream(request.selection.clone()).validate()?;
            let mut follower = storage.follow::<StoredRow>(request.after.as_ref(), request.selection.clone()).await?;
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
