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
use crate::storage::{StorageCursor, StoredRow};

use crate::storage::StorageManager;
use crate::storage::follow::Stream;

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowStream;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowStream {
    type Request = Stream;
    type Response = StorageCursor;
    const ID: &'static str = "storage.follow_stream";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<StoredRow>()))
    }
}

impl OutputDeclaration for FollowStream {
    type Item = StoredRow;
}

impl WireOperation for FollowStream {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(StoredRow::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = FollowStream;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<StoredRow>;

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Stream,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        crate::storage::query::Source::Stream(request.selection.clone()).validate()?;
        let mut follower = self
            .storage
            .follow::<StoredRow>(request.after.as_ref(), request.selection.clone())
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
