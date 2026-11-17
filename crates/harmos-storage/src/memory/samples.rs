/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Sample Batches and Materialize Row Views on Demand.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::ops::{Index, RangeToInclusive};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

// Third Party Imports

// Local Imports
use super::{Backend, RowStore, failure};
use crate::writes::SampleAppend;
use crate::{BackendFailure, MutationReceipt, Result, StorageFuture, StoredRow};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct SampleStore {
    rows: BTreeMap<u64, StoredRow>,
    columns: BTreeMap<u64, Arc<SampleAppend>>,
    materialized: OnceLock<BTreeMap<u64, StoredRow>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl SampleStore {
    fn materialize(&self) -> Result<&BTreeMap<u64, StoredRow>> {
        if let Some(rows) = self.materialized.get() {
            return Ok(rows);
        }
        let mut rows = self.rows.clone();
        for append in self.columns.values() {
            for row in append.rows()? {
                rows.insert(row.record.position.sequence, row);
            }
        }
        let _ = self.materialized.set(rows);
        Ok(self.materialized.get().expect("sample row image initialized"))
    }

    pub fn values(&self) -> Result<impl DoubleEndedIterator<Item = &StoredRow>> {
        Ok(self.materialize()?.values())
    }
    pub fn range(&self, range: RangeToInclusive<u64>) -> Result<impl Iterator<Item = (&u64, &StoredRow)>> {
        Ok(self.materialize()?.range(range))
    }
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &StoredRow)> {
        self.materialize().expect("validated sample row image").iter()
    }
    pub fn keys(&self) -> impl Iterator<Item = &u64> {
        self.materialize().expect("validated sample row image").keys()
    }
    pub fn len(&self) -> usize {
        self.rows.len() + self.columns.values().map(|append| append.batch.data.as_record_batch().num_rows()).sum::<usize>()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.columns.is_empty()
    }
    pub fn contains_key(&self, position: &u64) -> bool {
        self.rows.contains_key(position)
            || self.columns.range(..=position).next_back().is_some_and(|(_, append)| *position <= append.through.sequence)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&u64, &StoredRow) -> bool) -> Result<()> {
        let mut rows = self.materialize()?.clone();
        rows.retain(|position, row| keep(position, row));
        *self = rows.into_iter().collect();
        Ok(())
    }
}

impl RowStore<StoredRow> for SampleStore {
    fn get(&self, position: &u64) -> Option<&StoredRow> {
        self.rows.get(position)
    }
    fn insert(&mut self, position: u64, value: StoredRow) -> Result<()> {
        if self.columns.range(..=position).next_back().is_some_and(|(_, append)| position <= append.through.sequence) {
            return Err(failure(BackendFailure::Conflict));
        }
        self.materialized.take();
        self.rows.insert(position, value);
        Ok(())
    }
}

impl Index<&u64> for SampleStore {
    type Output = StoredRow;
    fn index(&self, position: &u64) -> &Self::Output {
        &self.materialize().expect("validated sample row image")[position]
    }
}

impl FromIterator<(u64, StoredRow)> for SampleStore {
    fn from_iter<T: IntoIterator<Item = (u64, StoredRow)>>(rows: T) -> Self {
        Self {
            rows: rows.into_iter().collect(),
            ..Self::default()
        }
    }
}

impl Backend {
    pub(super) fn commit_sample_columns<'a>(&'a self, append: &'a SampleAppend) -> StorageFuture<'a, MutationReceipt> {
        Box::pin(async move {
            self.history.check(&append.first.history)?;
            self.history.check(&append.through.history)?;
            // The Reference Backend's I/O Gate Sees One Columnar Unit.
            self.control.write([append.through.sequence]).await?;
            self.control.flush(append.through.sequence).await?;
            let mut receipts = self.mutation_receipts.lock().unwrap();
            if let Some(receipt) = receipts.get(&append.mutation().mutation) {
                append.replay(receipt)?;
                return Ok(receipt.clone());
            }
            let _transaction = self.conditional.lock().unwrap();
            let saved = self.saved.load(Ordering::SeqCst) as u64;
            if saved.checked_add(1) != Some(append.first.sequence) {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut rows = self.rows.lock().unwrap();
            if rows.contains_key(&append.first.sequence) || rows.contains_key(&append.through.sequence) {
                return Err(failure(BackendFailure::Conflict));
            }
            rows.materialized.take();
            rows.columns.insert(append.first.sequence, Arc::new(append.clone()));
            let receipt = append.receipt();
            receipts.insert(receipt.mutation.clone(), receipt.clone());
            self.saved.store(append.through.sequence as usize, Ordering::SeqCst);
            self.control.committed_bytes.lock().unwrap().push(append.batch.data.ipc_bytes().len());
            if self.control.flush_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(receipt)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
