/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Declarative Storage QueryRequest: A Source At A Full Cursor, Ordered Transforms
// And A Final Row Slice. Results Cross The Wire As Arrow IPC.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::StorageCursor;
use crate::model::storage::codec::{Nested, Repeated, product};
use crate::model::storage::query::transform::Transform;
use crate::model::storage::query::{QueryCompletion, QueryOutput, RowSlice, Source};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(QueryRequest {
    at: Nested<StorageCursor> => 1,
    source: Nested<Source> => 2,
    transforms: Repeated<Transform> => 3,
    slice: Nested<RowSlice> => 4,
});

pub struct Query;

impl crate::operations::OperationDeclaration for Query {
    type Request = QueryRequest;
    type Response = QueryCompletion;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<QueryOutput>;
    const ID: &'static str = "storage_query";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "Query"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        Some(crate::plugin::Grant::of(Self::ID, request.source.grant_scope()))
    }
}

impl crate::operations::RequestFor for QueryRequest {
    type Operation = Query;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct QueryRequest {
    pub at: StorageCursor,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub slice: RowSlice,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for QueryRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.at.memory_size().saturating_sub(std::mem::size_of_val(&self.at)))
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.transforms.memory_size().saturating_sub(std::mem::size_of_val(&self.transforms)))
            .saturating_add(self.slice.memory_size().saturating_sub(std::mem::size_of_val(&self.slice)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
