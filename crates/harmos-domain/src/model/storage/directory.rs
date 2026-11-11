/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Directory Discovery Contract

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::storage::directory_event::Event as DirectoryEvent;
pub use harmos_protocol::storage::{DirectoryCompletion, DirectoryEntry};

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
// Third Party Imports
// Local Imports
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryRule {
    pub pattern: String,
    pub include: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectorySelection {
    pub root: PathBuf,
    pub recursive: bool,
    pub rules: Vec<DirectoryRule>,
}

/* === Traits === */

/* === Implementations === */

impl MemorySize for DirectoryRule {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.pattern.capacity())
    }
}
impl MemorySize for DirectorySelection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.root.capacity())
            .saturating_add(self.rules.memory_size().saturating_sub(std::mem::size_of_val(&self.rules)))
    }
}
impl MemorySize for DirectoryEntry {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.path.capacity())
    }
}
impl MemorySize for DirectoryCompletion {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl MemorySize for DirectoryEvent {
    fn memory_size(&self) -> usize {
        let extra = match self {
            Self::Existing(entry) | Self::Created(entry) | Self::Modified(entry) => entry.path.capacity(),
            Self::Removed(path) => path.capacity(),
            Self::Renamed(rename) => rename.from.capacity().saturating_add(rename.to.capacity()),
            Self::InitialScanComplete(_) | Self::RescanRequired(_) => 0,
        };
        std::mem::size_of::<Self>().saturating_add(extra)
    }
}

impl TryFrom<harmos_protocol::storage::DirectorySelection> for DirectorySelection {
    type Error = crate::Error;
    fn try_from(value: harmos_protocol::storage::DirectorySelection) -> crate::Result<Self> {
        Ok(Self {
            root: PathBuf::from(value.root),
            recursive: value.recursive,
            rules: value
                .rules
                .into_iter()
                .map(|rule| DirectoryRule {
                    pattern: rule.pattern,
                    include: rule.include,
                })
                .collect(),
        })
    }
}
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
