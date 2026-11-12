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
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use arrow_array::RecordBatch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::columnar::{self, ArrowScan, Cancellation, ColumnSource};
use super::scalar::ScalarValue;
use crate::{BackendSnapshot, Error, Identifier, ReadLimits, Result, StorageBackend};

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
    pub name: Identifier,
    pub lower: Option<Vec<u8>>,
    pub upper: Option<Vec<u8>>,
    pub prefix: Vec<u8>,
    pub descending: bool,
    pub after: Option<IndexSeek>,
    pub limits: ReadLimits,
    pub work: ReadLimits,
}

pub struct IndexPage {
    pub entries: Vec<IndexEntry>,
    pub next: Option<IndexSeek>,
    pub complete: bool,
}

#[derive(Default)]
pub struct IndexCatalog {
    definitions: BTreeMap<Identifier, (IndexDefinition, u64)>,
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

impl IndexCatalog {
    pub fn register(&mut self, definition: &IndexDefinition, rebuild: bool) -> Result<u64> {
        definition.validate()?;
        let mut revision = 1;
        if let Some((prior, current)) = self.definitions.get(&definition.name) {
            if prior == definition {
                return Ok(*current);
            }
            if !rebuild {
                return Err(conflict("changed index requires explicit rebuild"));
            }
            revision = current.checked_add(1).ok_or_else(|| columnar::invalid("index revision overflow"))?;
        }
        self.definitions.insert(definition.name.clone(), (definition.clone(), revision));
        Ok(revision)
    }
    pub fn definition(&self, name: &Identifier) -> Result<(IndexDefinition, u64)> {
        self.definitions.get(name).cloned().ok_or_else(|| Error::IndexNotFound { name: name.clone() })
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
        if let Some(after) = &self.after
            && (after.snapshot != snapshot.token || after.revision != revision || after.plan != self.identity(definition)?)
        {
            return Err(conflict("index continuation snapshot, revision or plan changed"));
        }

        Ok(())
    }
    pub(crate) fn accepts(&self, entry: &IndexEntry) -> bool {
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

pub async fn list_index_entries(
    backend: &dyn StorageBackend, snapshot: &BackendSnapshot, read: &IndexRead, cancel: &Cancellation,
) -> Result<IndexPage> {
    cancel.check()?;
    let (definition, revision) = backend.get_index_definition(&read.name).await?;
    read.validate(snapshot, &definition, revision)?;
    if backend.get_storage_capabilities().native_index {
        return backend.list_index_entries(snapshot, read, cancel).await;
    }
    let mut scan = ArrowScan {
        source: definition.source.clone(),
        columns: definition.columns.clone(),
        predicate: None,
        time: None,
        start: 0,
        after: None,
        limits: read.work,
        work: read.work,
    };
    if !scan.columns.iter().any(|name| name == "_storage.position") {
        scan.columns.push("_storage.position".into());
    }
    let mut candidates = BTreeSet::new();
    let mut rows = 0usize;
    let mut bytes = 0usize;
    loop {
        cancel.check()?;
        if rows == read.work.rows.get() || bytes >= read.work.bytes.get() {
            return Err(columnar::exhausted());
        }
        scan.work = ReadLimits {
            rows: std::num::NonZeroUsize::new(read.work.rows.get() - rows).unwrap(),
            bytes: std::num::NonZeroUsize::new(read.work.bytes.get() - bytes).unwrap(),
        };
        scan.limits = scan.work;
        let page = columnar::list_column_rows(backend, snapshot, &scan, cancel).await?;
        rows += page.scanned;
        bytes = bytes.saturating_add(page.bytes_read);
        for row in 0..page.batch.num_rows() {
            if row % 256 == 0 {
                cancel.check()?;
            }
            let entry = entry(&page.batch, &definition, row)?;
            if !read.accepts(&entry) {
                continue;
            }
            candidates.insert(entry);
            if candidates.len() > read.limits.rows.get().saturating_add(1) {
                if read.descending {
                    candidates.pop_first();
                } else {
                    candidates.pop_last();
                }
            }
            let retained = candidates.iter().map(|entry| entry.key.capacity() + std::mem::size_of::<IndexEntry>()).sum::<usize>();
            if bytes.saturating_add(retained) > read.work.bytes.get() {
                return Err(columnar::exhausted());
            }
        }
        if page.complete {
            break;
        }
        scan.after = page.next;
    }
    finish(snapshot, read, &definition, revision, candidates.into_iter().collect())
}

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

pub(crate) fn entry(batch: &RecordBatch, definition: &IndexDefinition, row: usize) -> Result<IndexEntry> {
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

pub(crate) fn finish(
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

pub(crate) fn append_text(encoded: &mut Vec<u8>, value: &str) {
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
