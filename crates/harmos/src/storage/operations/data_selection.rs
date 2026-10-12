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
use std::time::SystemTime;

// Third Party Imports

// Local Imports
use super::Selection;
use crate::storage::SchemaKey;
use crate::storage_codec::{Id, Nested, Optional, Time, product};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSelection {
    pub selection: Selection,
    pub schema: SchemaKey,
    pub stream: Option<Identifier>,
    pub source_start: Option<SystemTime>,
    pub source_end: Option<SystemTime>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
product!(DataSelection {
    selection: Nested<Selection> => 1,
    schema: Nested<SchemaKey> => 2,
    stream: Optional<Id> => 3,
    source_start: Optional<Time> => 4,
    source_end: Optional<Time> => 5,
});

impl DataSelection {
    pub fn validate(&self) -> Result<()> {
        self.selection.validate()?;
        match (self.source_start, self.source_end) {
            (None, None) => Ok(()),
            (Some(start), Some(end)) if start < end => Ok(()),
            _ => Err(Error::InvalidInput {
                field: "storage source time",
                reason: "provide both bounds in increasing order",
            }),
        }
    }

    #[cfg(feature = "runtime")]
    pub(super) fn source_time(&self) -> Option<crate::storage::SourceTimeRange> {
        self.source_start
            .zip(self.source_end)
            .map(|(start, end)| crate::storage::SourceTimeRange { start, end })
    }
}

impl MemorySize for DataSelection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.selection
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.selection)),
            )
            .saturating_add(
                self.schema
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.schema)),
            )
            .saturating_add(
                self.stream
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.stream)),
            )
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "runtime")]
pub(super) fn decode(bytes: &[u8], allowance: std::num::NonZeroUsize) -> Result<Vec<u8>> {
    let required = std::mem::size_of::<Vec<u8>>().saturating_add(bytes.len());
    if required > allowance.get() {
        return Err(Error::InvalidInput {
            field: "storage payload",
            reason: "payload exceeds decoded allowance",
        });
    }
    Ok(Box::<[u8]>::from(bytes).into_vec())
}

/* === Tests === */
