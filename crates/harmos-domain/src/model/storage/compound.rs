/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compound Atomic Mutation Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::{ObjectKey, ObjectValue, ObjectVersionToken, RecordInput, StorageCursor};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// One durable outcome, including refused conditions, replayed under its mutation identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompoundReceipt {
    pub position: StorageCursor,
    pub outcome: CompoundOutcome,
}

/* === Enums === */

/// Ordered items; a failed condition refuses the entire commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationItem {
    Create(ObjectValue),
    Set {
        expected: ObjectVersionToken,
        value: ObjectValue,
    },
    Delete {
        schema: Identifier,
        item: ObjectKey,
        expected: Option<ObjectVersionToken>,
    },
    Append(RecordInput),

    /// Delete One Opaque Scope as the Sole Item of a Mutation.
    DeleteScope(Identifier),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompoundOutcome {
    Applied {
        positions: Vec<StorageCursor>,
    },

    /// Get the conflicting object again and retry with a new mutation identity.
    Conflict {
        index: usize,
        current: Option<ObjectVersionToken>,
    },
}

/* === Traits === */

/* === Implementations === */

impl MemorySize for MutationItem {
    fn memory_size(&self) -> usize {
        let retained = match self {
            Self::Create(value) => value.memory_size(),
            Self::Set { expected, value } => expected.memory_size().saturating_add(value.memory_size()),
            Self::Delete { schema, item, expected } => {
                schema.memory_size().saturating_add(item.memory_size()).saturating_add(expected.memory_size())
            }
            Self::Append(record) => record.memory_size(),
            Self::DeleteScope(scope) => scope.memory_size(),
        };
        size_of::<Self>().saturating_add(retained)
    }
}

impl MemorySize for CompoundReceipt {
    fn memory_size(&self) -> usize {
        size_of::<Self>().saturating_add(self.position.memory_size()).saturating_add(self.outcome.memory_size())
    }
}

impl MemorySize for CompoundOutcome {
    fn memory_size(&self) -> usize {
        size_of::<Self>().saturating_add(match self {
            Self::Applied { positions } => positions.memory_size(),
            Self::Conflict { current, .. } => current.memory_size(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
