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
use crate::model::storage::{MutationId, StorageCursor};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(SaveRequest {
    mutation: crate::model::storage::codec::Nested<MutationId> => 1,
    at: crate::model::storage::codec::Nested<StorageCursor> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    pub mutation: MutationId,
    pub at: StorageCursor,
}

pub struct Save;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for SaveRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.at.memory_size()
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
