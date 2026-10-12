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
#[cfg(feature = "runtime")]
use std::sync::Arc;

// Third Party Imports

// Local Imports
#[cfg(feature = "runtime")]
use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};
#[cfg(feature = "runtime")]
use crate::storage::StorageManager;
use crate::storage::{MutationId, SchemaInput, StorageCursor};
#[cfg(feature = "runtime")]
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage_codec::product!(WriteSchemaRequest {
    mutation: crate::storage_codec::Nested<MutationId> => 1,
    schema: crate::storage_codec::Nested<SchemaInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSchemaRequest {
    pub mutation: MutationId,
    pub schema: SchemaInput,
}
pub struct WriteSchema;
#[cfg(feature = "runtime")]
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
    const VERSION: u32 = 1;
}
impl crate::MemorySize for WriteSchemaRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.schema.memory_size()
            - std::mem::size_of::<SchemaInput>()
    }
}
#[cfg(feature = "runtime")]
impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteSchema;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();
    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut WriteSchemaRequest,
        _: &mut Self::Endpoints,
    ) -> Result<StorageCursor> {
        self.storage
            .write_schema_once(
                &request.mutation,
                &request.schema.key,
                request.schema.fields(),
            )
            .await
    }
    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
