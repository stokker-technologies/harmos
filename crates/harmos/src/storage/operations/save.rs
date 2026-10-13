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

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

use crate::storage::StorageManager;
use crate::storage::{MutationId, StorageCursor};

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(SaveRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    through: crate::storage::codec::Nested<StorageCursor> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    pub mutation: MutationId,
    pub through: StorageCursor,
}

pub struct Save;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for SaveRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.through.memory_size()
            - std::mem::size_of::<StorageCursor>()
    }
}

impl OperationDeclaration for Save {
    type Request = SaveRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.save";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for Save {
    const VERSION: u32 = 1;
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = Save;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut SaveRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            self.storage
                .save_once(&request.mutation, &request.through)
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
