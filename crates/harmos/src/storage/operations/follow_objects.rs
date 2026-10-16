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

use crate::storage::StorageManager;
use crate::storage::follow::Objects;
use crate::storage::{ObjectEntry, StorageCursor};

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowObjects;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowObjects {
    type Request = Objects;
    type Response = StorageCursor;
    const ID: &'static str = "storage.follow_objects";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<ObjectEntry>()))
    }
}

impl OutputDeclaration for FollowObjects {
    type Item = ObjectEntry;
}

impl WireOperation for FollowObjects {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(ObjectEntry::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = FollowObjects;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<ObjectEntry>;

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Objects,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        if !request.selection.follows_revisions() {
            return Err(Error::InvalidInput {
                field: "storage follow",
                reason: "following delivers every object revision",
            });
        }
        let mut follower = self
            .storage
            .follow::<ObjectEntry>(request.after.as_ref(), request.selection.clone())
            .await?;
        loop {
            let page = follower.next().await?;
            for entry in page.entries() {
                output
                    .send_with(entry.memory_size(), || Ok(entry.clone()))
                    .await?;
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
