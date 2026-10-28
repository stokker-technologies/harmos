/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Directory Discovery Contract

use std::path::PathBuf;

use super::codec::{Flag, Nested, Repeated, Text, Unsigned, product, variants};
use super::files::codec::LocalPath;
use crate::MemorySize;

product!(DirectoryRule { pattern: Text => 1, include: Flag => 2 });
product!(DirectorySelection { root: LocalPath => 1, recursive: Flag => 2, rules: Repeated<DirectoryRule> => 3 });
product!(DirectoryEntry { path: Text => 1, directory: Flag => 2, length: Unsigned => 3 });
product!(DirectoryCompletion { entries: Unsigned => 1 });
variants!(DirectoryEvent {
    0, 1, "existing": Self::Existing(entry) => { entry: Nested<DirectoryEntry> => 1 } => Self::Existing(entry);
    1, 2, "initial_scan_complete": Self::InitialScanComplete => {} => Self::InitialScanComplete;
    2, 3, "created": Self::Created(entry) => { entry: Nested<DirectoryEntry> => 1 } => Self::Created(entry);
    3, 4, "modified": Self::Modified(entry) => { entry: Nested<DirectoryEntry> => 1 } => Self::Modified(entry);
    4, 5, "removed": Self::Removed { path } => { path: Text => 1 } => Self::Removed { path };
    5, 6, "renamed": Self::Renamed { from, to } => { from: Text => 1, to: Text => 2 } => Self::Renamed { from, to };
    6, 7, "rescan_required": Self::RescanRequired => {} => Self::RescanRequired;
});

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub path: String,
    pub directory: bool,
    pub length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectoryCompletion {
    pub entries: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectoryEvent {
    Existing(DirectoryEntry),
    InitialScanComplete,
    Created(DirectoryEntry),
    Modified(DirectoryEntry),
    Removed { path: String },
    Renamed { from: String, to: String },
    RescanRequired,
}

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
            Self::Removed { path } => path.capacity(),
            Self::Renamed { from, to } => from.capacity().saturating_add(to.capacity()),
            Self::InitialScanComplete | Self::RescanRequired => 0,
        };
        std::mem::size_of::<Self>().saturating_add(extra)
    }
}
