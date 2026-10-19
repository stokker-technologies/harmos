/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Declarative Storage Query: A Source At A Full Cursor, Ordered Transforms
// And A Final Row Slice. Results Cross The Wire As Arrow IPC.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::model::storage::StorageCursor;
use crate::model::storage::codec::{Nested, Repeated, product};
use crate::model::storage::query::transform::Transform;
use crate::model::storage::query::{QueryCompletion, QueryOutput, RowSlice, Source};
use crate::operations::{ReplaySafety, Visibility};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

product!(Query {
    at: Nested<StorageCursor> => 1,
    source: Nested<Source> => 2,
    transforms: Repeated<Transform> => 3,
    slice: Nested<RowSlice> => 4,
});

crate::declare!(Query {
    id: "storage.query",
    version: 1,
    policy: IO_POLICY,
    response: QueryCompletion,
    output: QueryOutput,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Idempotent,
    grpc: ("harmos.storage.v1", "Storage", "Query"),
    grant: |request: &Query| Some(crate::plugin::Grant::of("storage.query", request.source.grant_scope(),)),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Query {
    pub at: StorageCursor,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub slice: RowSlice,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Query {
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        if self.transforms.len() > 64 {
            return Err(crate::model::storage::query::transform::invalid("too many query transforms"));
        }
        Ok(())
    }
}

impl MemorySize for Query {
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
