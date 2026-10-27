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
use harmos_domain::model::storage::{ObjectEntry, ObjectSelection, StorageCursor};
use harmos_domain::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowObjectsRequest {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<ObjectSelection> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowObjectsRequest {
    pub after: Option<StorageCursor>,
    pub selection: ObjectSelection,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_follow_objects",
    request = FollowObjectsRequest,
    response = StorageCursor,
    policy = STREAM_POLICY,
    infer_request,
    output = ObjectEntry,
    visibility = Visibility::Host)]
pub struct FollowObjects {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowObjectsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::{Operation, StreamOutput};

    use harmos_storage::StorageManager;
    use harmos_storage::{ObjectEntry, StorageCursor};

    use crate::{Error, MemorySize, Result};

    impl Operation for super::FollowObjects {
        type Request = <FollowObjects as harmos_domain::OperationDeclaration>::Request;
        type Response = <FollowObjects as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<ObjectEntry>;

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<StorageCursor>> {
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
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
