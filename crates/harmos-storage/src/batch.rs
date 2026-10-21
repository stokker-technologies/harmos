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
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use super::manager::encoded_size;
use super::{FileReference, ObjectEntry, StorageCursor, StorageSchema, StoredRecord, StoredRow};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct StorageBatch<'a> {
    parts: &'a [StoragePart<'a>],
    first: &'a StorageCursor,
    through: &'a StorageCursor,
    len: usize,
    bytes: usize,
}

/* === Enums === */

#[derive(Clone, Copy)]
pub enum StoragePart<'a> {
    Schemas(&'a [StorageSchema]),
    Objects(&'a [ObjectEntry]),
    Records(&'a [StoredRecord]),
    Stream(&'a [StoredRow]),
    Files(&'a [FileReference]),
}

#[derive(Clone, Copy)]
pub enum StorageEntry<'a> {
    Schema(&'a StorageSchema),
    Object(&'a ObjectEntry),
    Record(&'a StoredRecord),
    Row(&'a StoredRow),
    File(&'a FileReference),
}

/* === Traits === */

/* === Implementations === */

impl<'a> StorageBatch<'a> {
    pub fn new(parts: &'a [StoragePart<'a>], max_bytes: NonZeroUsize) -> Result<Self> {
        let mut entries = parts.iter().copied().flat_map(StoragePart::entries);
        let first = entries.next().ok_or(invalid("batch is empty"))?;
        let mut through = first.position();
        if through.sequence == 0 {
            return Err(invalid("position zero is not a data item"));
        }
        let mut bytes = first.encoded_bytes()?;
        let mut len = 1usize;
        for entry in entries {
            through.history.check(&entry.position().history)?;
            if through.sequence.checked_add(1) != Some(entry.position().sequence) {
                return Err(invalid("batch positions are not contiguous"));
            }
            bytes = bytes.checked_add(entry.encoded_bytes()?).ok_or(invalid("batch byte count overflow"))?;
            if bytes > max_bytes.get() {
                return Err(invalid("batch exceeds the encoded byte bound"));
            }
            len = len.checked_add(1).ok_or(invalid("batch entry count overflow"))?;
            through = entry.position();
        }
        if bytes > max_bytes.get() {
            return Err(invalid("batch exceeds the encoded byte bound"));
        }
        Ok(Self { parts, first: first.position(), through, len, bytes })
    }

    pub fn parts(&self) -> impl Iterator<Item = StoragePart<'a>> + '_ {
        self.parts.iter().copied().filter_map(|part| {
            let start = part.partition(self.first.sequence, false);
            let end = part.partition(self.through.sequence, true);
            (end > start).then(|| part.slice(start, end))
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = StorageEntry<'a>> + '_ {
        self.parts().flat_map(StoragePart::entries)
    }

    pub fn first(&self) -> &'a StorageCursor {
        self.first
    }

    pub fn through(&self) -> &'a StorageCursor {
        self.through
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn encoded_bytes(&self) -> usize {
        self.bytes
    }
}

impl<'a> StoragePart<'a> {
    // Admitted spans are in cursor order. Seek a saved prefix logarithmically;
    // retrying a large group must not rescan its whole saved prefix each time.
    fn partition(self, sequence: u64, inclusive: bool) -> usize {
        let mut start = 0;
        let mut end = self.len();
        while start < end {
            let middle = start + (end - start) / 2;
            let position = self.slice(middle, middle + 1).entries().next().unwrap().position().sequence;
            if position < sequence || (inclusive && position == sequence) {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        start
    }

    fn slice(self, start: usize, end: usize) -> Self {
        match self {
            Self::Schemas(values) => Self::Schemas(&values[start..end]),
            Self::Objects(values) => Self::Objects(&values[start..end]),
            Self::Records(values) => Self::Records(&values[start..end]),
            Self::Stream(values) => Self::Stream(&values[start..end]),
            Self::Files(values) => Self::Files(&values[start..end]),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Schemas(values) => values.len(),
            Self::Objects(values) => values.len(),
            Self::Records(values) => values.len(),
            Self::Stream(values) => values.len(),
            Self::Files(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(self) -> impl Iterator<Item = StorageEntry<'a>> + 'a {
        (0..self.len()).map(move |index| match self {
            Self::Schemas(values) => StorageEntry::Schema(&values[index]),
            Self::Objects(values) => StorageEntry::Object(&values[index]),
            Self::Records(values) => StorageEntry::Record(&values[index]),
            Self::Stream(values) => StorageEntry::Row(&values[index]),
            Self::Files(values) => StorageEntry::File(&values[index]),
        })
    }
}

impl<'a> StorageEntry<'a> {
    pub fn position(self) -> &'a StorageCursor {
        match self {
            Self::Schema(value) => &value.position,
            Self::Object(value) => &value.position,
            Self::Record(value) => &value.position,
            Self::Row(value) => &value.record.position,
            Self::File(value) => &value.position,
        }
    }

    pub(super) fn encoded_bytes(self) -> Result<usize> {
        match self {
            Self::Schema(value) => value.encoded_bytes(),
            Self::Object(value) => {
                value.version.check_position(&value.position)?;
                encoded_size(value)
            }
            Self::Record(value) => encoded_size(value),
            Self::Row(value) => encoded_size(value),
            Self::File(value) => encoded_size(value),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage batch", reason }
}

/* === Tests === */
