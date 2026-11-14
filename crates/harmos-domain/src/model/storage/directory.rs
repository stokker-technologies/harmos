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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirectoryEntry {
    pub path: String,
    pub directory: bool,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirectoryCompletion {
    pub entries: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirectoryRename {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DirectoryEvent {
    Existing(DirectoryEntry),
    Created(DirectoryEntry),
    Modified(DirectoryEntry),
    Removed(String),
    Renamed(DirectoryRename),
    InitialScanComplete(()),
    RescanRequired(()),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirectoryRule {
    pub pattern: String,
    pub include: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
