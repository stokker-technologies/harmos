/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Indexed Record Images

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    ops::Bound::{Excluded, Included},
    sync::Arc,
};

// Third Party Imports

// Local Imports
use super::{RecordView, Result, SchemaKey, StoredRecord};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct RecordStore {
    values: Arc<BTreeMap<u64, RecordView>>,
    ranks: Arc<BTreeMap<String, Arc<Vec<u64>>>>,
}

/* === Enums === */

/* === Traits === */

pub(super) trait RowStore<T> {
    fn get(&self, position: &u64) -> Option<&T>;
    fn insert(&mut self, position: u64, value: T) -> Result<()>;
}

/* === Implementations === */

impl<T> RowStore<T> for BTreeMap<u64, T> {
    fn get(&self, position: &u64) -> Option<&T> {
        BTreeMap::get(self, position)
    }
    fn insert(&mut self, position: u64, value: T) -> Result<()> {
        BTreeMap::insert(self, position, value);
        Ok(())
    }
}

impl RowStore<StoredRecord> for RecordStore {
    fn get(&self, position: &u64) -> Option<&StoredRecord> {
        self.get(position)
    }
    fn insert(&mut self, position: u64, value: StoredRecord) -> Result<()> {
        if self.get(&position) == Some(&value) {
            return Ok(());
        }
        let key = serde_json::to_string(&value.schema).unwrap();
        let view = RecordView::new(value)?;
        let ranks = Arc::make_mut(&mut self.ranks).entry(key).or_default();
        let ranks = Arc::make_mut(ranks);
        if let Err(index) = ranks.binary_search(&position) {
            ranks.insert(index, position);
        }
        Arc::make_mut(&mut self.values).insert(position, view);
        Ok(())
    }
}

impl RecordStore {
    pub(super) fn shares(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.values, &other.values) && Arc::ptr_eq(&self.ranks, &other.ranks)
    }
    pub(super) fn seek(&self, schema: &SchemaKey, offset: u64, cut: u64) -> u64 {
        if offset == 0 {
            return 0;
        }
        let key = serde_json::to_string(schema).unwrap();
        let Some(ranks) = self.ranks.get(&key) else {
            return cut;
        };
        let end = ranks.partition_point(|entry| *entry <= cut);
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset >= end {
            return cut;
        }
        ranks[offset - 1]
    }

    pub(super) fn selected(&self, schema: &SchemaKey, after: u64, cut: u64) -> impl Iterator<Item = (&StoredRecord, usize)> {
        self.selected_views(schema, after, cut).map(|view| (view.record(), view.encoded_bytes()))
    }

    pub(super) fn selected_views(&self, schema: &SchemaKey, after: u64, cut: u64) -> impl Iterator<Item = &RecordView> {
        let key = serde_json::to_string(schema).unwrap();
        let mut contiguous = (self.ranks.len() == 1 && after < cut).then(|| self.values.range((Excluded(after), Included(cut))));
        self.ranks
            .get(&key)
            .into_iter()
            .flat_map(move |ranks| {
                let start = ranks.partition_point(|position| *position <= after);
                let end = ranks.partition_point(|position| *position <= cut);
                ranks[start..end.max(start)].iter()
            })
            .map(move |position| match &mut contiguous {
                Some(rows) => rows.next().map(|(_, row)| row).expect("ranked record exists"),
                None => self.values.get(position).expect("ranked record exists"),
            })
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &u64> {
        self.values.keys()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn contains_key(&self, position: &u64) -> bool {
        self.values.contains_key(position)
    }
    pub fn get(&self, position: &u64) -> Option<&StoredRecord> {
        self.values.get(position).map(RecordView::record)
    }
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &StoredRecord> {
        self.values.values().map(RecordView::record)
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&u64, &StoredRecord)> {
        self.values.iter().map(|(position, view)| (position, view.record()))
    }
}

impl std::ops::Index<&u64> for RecordStore {
    type Output = StoredRecord;
    fn index(&self, position: &u64) -> &Self::Output {
        self.values[position].record()
    }
}

impl FromIterator<(u64, StoredRecord)> for RecordStore {
    fn from_iter<T: IntoIterator<Item = (u64, StoredRecord)>>(rows: T) -> Self {
        let mut records = Self::default();
        for (position, row) in rows {
            RowStore::insert(&mut records, position, row).expect("restored fixture record has a serializable page size");
        }
        records
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
