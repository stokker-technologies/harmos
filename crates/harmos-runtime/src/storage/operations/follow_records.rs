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

use crate::operations::{Operation, StreamOutput};

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
    type Request = <FollowRecords as harmos_domain::OperationDeclaration>::Request;
    type Response = <FollowRecords as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<Followed>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
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
