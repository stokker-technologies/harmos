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




// Local Imports
use super::columnar;
use harmos_query::columnar::{ArrowScan, Cancellation};
use harmos_query::index::*;

use crate::{BackendSnapshot, Error, Identifier, ReadLimits, Result, StorageBackend};



#[derive(Default)]
pub struct IndexCatalog {
    definitions: BTreeMap<Identifier, (IndexDefinition, u64)>,
}

impl IndexCatalog {
    pub fn register(&mut self, definition: &IndexDefinition, expected: Option<u64>, rebuild: bool) -> Result<u64> {
        definition.validate()?;
        if self.definitions.get(&definition.name).map(|(_, revision)| *revision) != expected {
            return Err(conflict("set index definition: get again and retry"));
        }
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

pub async fn list_index_entries(
    backend: &dyn StorageBackend, snapshot: &BackendSnapshot, read: &IndexRead, cancel: &Cancellation,
) -> Result<IndexPage> {
    cancel.check()?;
    let (definition, revision) = backend.get_index_definition(&read.name).await?;
    read.validate(snapshot, &definition, revision)?;
    if backend.capabilities().native_index {
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
    Ok(finish(snapshot, read, &definition, revision, candidates.into_iter().collect())?)
}
fn conflict(reason: &'static str) -> Error { Error::backend("update index", crate::BackendFailure::Conflict, std::io::Error::other(reason)) }
