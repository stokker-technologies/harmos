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
use std::collections::BTreeSet;

// Third Party Imports
use arrow_array::RecordBatch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::columnar::{self, ColumnSource};
use super::scalar::ScalarValue;
use crate::{Error, Identifier, Result};
use harmos_domain::model::storage::{BackendSnapshot, ReadLimits};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: Identifier,
    pub version: u32,
    pub source: ColumnSource,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IndexEntry {
    pub key: Vec<u8>,
    pub position: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSeek {
    pub snapshot: Vec<u8>,
    pub plan: [u8; 32],
    pub revision: u64,
    pub key: Vec<u8>,
    pub position: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexRead {
    pub revision: Option<u64>,
    pub name: Identifier,
    pub lower: Option<Vec<u8>>,
    pub upper: Option<Vec<u8>>,
    pub prefix: Vec<u8>,
    pub descending: bool,
    pub after: Option<IndexSeek>,
    pub limits: ReadLimits,
    pub work: ReadLimits,
}

#[derive(Debug)]
pub struct IndexPage {
    pub entries: Vec<IndexEntry>,
    pub next: Option<IndexSeek>,
    pub complete: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl IndexDefinition {
    pub fn validate(&self) -> Result<()> {
        if self.version == 0
            || self.columns.is_empty()
            || self.columns.len() > crate::arrow::MAX_FIELDS
            || self.columns.iter().any(String::is_empty)
            || self.columns.iter().collect::<BTreeSet<_>>().len() != self.columns.len()
        {
            return Err(columnar::invalid("index requires a nonzero version and unique bounded columns"));
        }
        Ok(())
    }
}

impl IndexRead {
    pub fn identity(&self, definition: &IndexDefinition) -> Result<[u8; 32]> {
        let data = serde_json::to_vec(&(definition, &self.lower, &self.upper, &self.prefix, self.descending))
            .map_err(|source| Error::backend("encode index plan", crate::BackendFailure::Permanent, source))?;
        Ok(Sha256::digest(data).into())
    }
    pub fn validate(&self, snapshot: &BackendSnapshot, definition: &IndexDefinition, revision: u64) -> Result<()> {
        if self.lower.as_ref().zip(self.upper.as_ref()).is_some_and(|(lower, upper)| lower >= upper) {
            return Err(columnar::invalid("index bounds must increase"));
        }
        if self.revision.is_some_and(|expected| expected != revision) {
            return Err(conflict("ListIndexEntries definition revision is unavailable; get again and retry"));
        }
        if let Some(after) = &self.after
            && (after.snapshot != snapshot.token || after.revision != revision || after.plan != self.identity(definition)?)
        {
            return Err(conflict("index continuation snapshot, revision or plan changed"));
        }

        Ok(())
    }
    pub fn accepts(&self, entry: &IndexEntry) -> bool {
        if self.lower.as_ref().is_some_and(|lower| &entry.key < lower)
            || self.upper.as_ref().is_some_and(|upper| &entry.key >= upper)
            || !entry.key.starts_with(&self.prefix)
        {
            return false;
        }
        self.after.as_ref().is_none_or(|after| {
            let order = (&entry.key, entry.position).cmp(&(&after.key, after.position));
            if self.descending { order.is_lt() } else { order.is_gt() }
        })
    }
}

/* === Public Functions === */

pub fn encode_key(values: &[ScalarValue]) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    for value in values {
        match value {
            ScalarValue::Null => encoded.push(0),
            ScalarValue::Bool(value) => encoded.extend([1, u8::from(*value)]),
            ScalarValue::Signed(value) => {
                encoded.push(2);
                encoded.extend(((*value as u64) ^ (1 << 63)).to_be_bytes());
            }
            ScalarValue::Unsigned(value) => {
                encoded.push(3);
                encoded.extend(value.to_be_bytes());
            }
            ScalarValue::Float(value) if value.is_finite() => {
                let bits = if *value == 0.0 { 0 } else { value.to_bits() };
                let ordered = if bits >> 63 == 0 { bits ^ (1 << 63) } else { !bits };
                encoded.push(4);
                encoded.extend(ordered.to_be_bytes());
            }
            ScalarValue::Text(value) => append_text(&mut encoded, value),
            ScalarValue::Datetime(value) => {
                let value = i64::try_from(*value).map_err(|_| columnar::invalid("index timestamp overflow"))?;
                encoded.push(6);
                encoded.extend(((value as u64) ^ (1 << 63)).to_be_bytes());
            }
            _ => return Err(columnar::invalid("index keys require finite primitive scalar values")),
        }
    }
    Ok(encoded)
}

/* === Private Functions === */

pub fn entry(batch: &RecordBatch, definition: &IndexDefinition, row: usize) -> Result<IndexEntry> {
    let values = definition
        .columns
        .iter()
        .map(|name| {
            let column = batch.column_by_name(name).ok_or_else(|| columnar::invalid("indexed column is unavailable"))?;
            super::transform::scalar(column.as_ref(), row)
        })
        .collect::<Result<Vec<_>>>()?;
    let positions = batch
        .column_by_name("_storage.position")
        .and_then(|array| array.as_any().downcast_ref::<arrow_array::UInt64Array>())
        .ok_or_else(|| columnar::invalid("indexed source has no positions"))?;
    Ok(IndexEntry {
        key: encode_key(&values)?,
        position: positions.value(row),
    })
}

pub fn finish(
    snapshot: &BackendSnapshot, read: &IndexRead, definition: &IndexDefinition, revision: u64, mut candidates: Vec<IndexEntry>,
) -> Result<IndexPage> {
    candidates.sort();
    if read.descending {
        candidates.reverse();
    }
    let mut entries = Vec::new();
    let mut bytes = 0usize;
    let mut complete = true;
    for entry in candidates {
        let size = entry.key.capacity() + std::mem::size_of::<IndexEntry>();
        if entries.len() == read.limits.rows.get() || bytes.saturating_add(size) > read.limits.bytes.get() {
            complete = false;
            break;
        }
        bytes += size;
        entries.push(entry);
    }
    if entries.is_empty() && !complete {
        return Err(columnar::exhausted());
    }
    let next = if complete {
        None
    } else {
        let entry = entries.last().expect("nonempty page");
        Some(IndexSeek {
            snapshot: snapshot.token.clone(),
            plan: read.identity(definition)?,
            revision,
            key: entry.key.clone(),
            position: entry.position,
        })
    };
    Ok(IndexPage { entries, next, complete })
}

pub fn append_text(encoded: &mut Vec<u8>, value: &str) {
    encoded.push(5);
    for byte in value.as_bytes() {
        encoded.push(*byte);
        if *byte == 0 {
            encoded.push(255);
        }
    }
    encoded.extend([0, 0]);
}

fn conflict(reason: &'static str) -> Error {
    Error::backend("storage index", crate::BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidInput, reason))
}

/* === Tests === */

impl crate::MemorySize for IndexDefinition {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(crate::MemorySize::memory_size(&self.name))
            .saturating_add(crate::MemorySize::memory_size(&self.source))
            .saturating_add(crate::MemorySize::memory_size(&self.columns))
    }
}

impl crate::MemorySize for IndexEntry {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(crate::MemorySize::memory_size(&self.key))
    }
}

impl crate::MemorySize for IndexPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(crate::MemorySize::memory_size(&self.entries))
    }
}
