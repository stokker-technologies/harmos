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

use harmos_domain::operations::storage::FollowStream;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Operation, StreamOutput};
use crate::storage::{StorageCursor, StoredRow};

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
    type Request = <FollowStream as harmos_domain::OperationDeclaration>::Request;
    type Response = <FollowStream as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<StoredRow>;

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        crate::storage::query::Source::Stream(request.selection.clone()).validate()?;
        let mut follower = self.storage.follow::<StoredRow>(request.after.as_ref(), request.selection.clone()).await?;
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
