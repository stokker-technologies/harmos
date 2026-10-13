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
use crate::storage::{MutationId, SchemaInput, StorageCursor};

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(WriteSchemaRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    schema: crate::storage::codec::Nested<SchemaInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSchemaRequest {
    pub mutation: MutationId,
    pub schema: SchemaInput,
}
pub struct WriteSchema;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteSchema {
    type Request = WriteSchemaRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.write_schema";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for WriteSchema {
    const VERSION: u32 = 2;
}
impl crate::MemorySize for WriteSchemaRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.schema.memory_size()
            - std::mem::size_of::<SchemaInput>()
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteSchema;
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
        request: &mut WriteSchemaRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            self.storage
                .write_schema_once(
                    &request.mutation,
                    &request.schema.key,
                    request.schema.graph(),
                )
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
