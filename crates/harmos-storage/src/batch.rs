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
use crate::manager::Inner;
use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
// Third Party Imports
// Local Imports
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type FileChunk = StorageBatch<u8>;

/* === Structs === */

#[derive(Debug)]
pub struct StorageBatch<T> {
    values: Arc<Vec<T>>,
    range: Range<usize>,
    charges: Arc<Vec<Arc<WriteCharge>>>,
    memory: Arc<Vec<crate::arrow::RetainedMemory>>,
}

#[derive(Debug)]
pub(crate) struct WriteCharge {
    bytes: usize,
    retired: AtomicBool,
    held: Arc<AtomicUsize>,
    changed: tokio::sync::watch::Sender<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T> StorageBatch<T> {
    pub fn new(values: Vec<T>) -> Self {
        let range = 0..values.len();
        Self { values: Arc::new(values), range, charges: Arc::new(Vec::new()), memory: Arc::new(Vec::new()) }
    }

    pub(crate) fn retained(mut self, charge: Arc<WriteCharge>) -> Self {
        let mut charges = self.charges.as_ref().clone();
        charges.push(charge);
        self.charges = Arc::new(charges);
        self
    }

    pub(crate) fn retain_memory(mut self, memory: crate::arrow::RetainedMemory) -> Self {
        Arc::make_mut(&mut self.memory).push(memory);
        self
    }

    pub(crate) fn charged(&self) -> bool {
        self.charges.iter().any(|charge| charge.retired.load(Ordering::SeqCst))
    }

    pub(crate) fn retire(&self) {
        for charge in self.charges.iter() {
            charge.retire();
        }
    }

    pub(crate) fn merge(batches: &mut [&mut Self]) -> Option<Self> {
        let first = batches.first()?;
        let mut range = first.range.clone();
        let adjoining = batches.iter().skip(1).all(|batch| {
            let joined = Arc::ptr_eq(&first.values, &batch.values) && range.end == batch.range.start;
            range.end = batch.range.end;
            joined
        });
        if adjoining {
            return Some(Self {
                values: Arc::clone(&first.values),
                range,
                charges: Arc::clone(&first.charges),
                memory: Arc::clone(&first.memory),
            });
        }
        if batches.iter().any(|batch| Arc::strong_count(&batch.values) != 1 || batch.range != (0..batch.values.len())) {
            return None;
        }
        let count = batches.iter().map(|batch| batch.len()).sum();
        let lengths: Vec<_> = batches.iter().map(|batch| batch.len()).collect();
        let charges = batches.iter().flat_map(|batch| batch.charges.iter().cloned()).collect();
        let memory = batches.iter().flat_map(|batch| batch.memory.iter().cloned()).collect();
        let mut values = Vec::with_capacity(count);
        for batch in batches.iter_mut() {
            let source = std::mem::take(Arc::get_mut(&mut batch.values).expect("unique queued backing"));
            values.extend(source);
        }
        let merged = Self { values: Arc::new(values), range: 0..count, charges: Arc::new(charges), memory: Arc::new(memory) };
        let mut start = 0;
        for (batch, length) in batches.iter_mut().zip(lengths) {
            **batch = merged.slice(start..start + length);
            start += length;
        }
        Some(merged)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values[self.range.clone()]
    }

    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(range.start <= range.end && range.end <= self.len(), "batch slice is outside its span");
        let range = self.range.start + range.start..self.range.start + range.end;
        Self { values: Arc::clone(&self.values), range, charges: Arc::clone(&self.charges), memory: Arc::clone(&self.memory) }
    }

    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.values.capacity()
    }

    pub fn allocation_bytes(&self) -> usize {
        self.values.capacity().saturating_mul(std::mem::size_of::<T>())
    }
}

impl WriteCharge {
    pub(crate) fn new(inner: &Arc<Inner>, bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            bytes,
            retired: AtomicBool::new(false),
            held: Arc::clone(&inner.retained_write_bytes),
            changed: inner.changed.clone(),
        })
    }

    pub(crate) fn read(inner: &Arc<Inner>, bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            bytes,
            retired: AtomicBool::new(false),
            held: Arc::clone(&inner.retained_read_bytes),
            changed: inner.changed.clone(),
        })
    }

    pub(crate) fn retire(&self) {
        if !self.retired.swap(true, Ordering::SeqCst) {
            self.held.fetch_add(self.bytes, Ordering::SeqCst);
        }
    }
}

impl Drop for WriteCharge {
    fn drop(&mut self) {
        if self.retired.load(Ordering::SeqCst) {
            self.held.fetch_sub(self.bytes, Ordering::SeqCst);
            self.changed.send_modify(|revision| *revision = revision.wrapping_add(1));
        }
    }
}

impl From<Vec<u8>> for StorageBatch<u8> {
    fn from(values: Vec<u8>) -> Self {
        Self::new(values)
    }
}

impl<T> Clone for StorageBatch<T> {
    fn clone(&self) -> Self {
        Self {
            values: Arc::clone(&self.values),
            range: self.range.clone(),
            charges: Arc::clone(&self.charges),
            memory: Arc::clone(&self.memory),
        }
    }
}

impl<T: MemorySize> MemorySize for StorageBatch<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.values.as_ref().memory_size()).saturating_add(2 * std::mem::size_of::<usize>())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{FileChunk, StorageBatch};
    use crate::MemorySize;

    #[test]
    fn chunk_allocation() {
        let mut bytes = Vec::with_capacity(1024);
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let batch = FileChunk::from(bytes);
        assert_eq!(batch.as_slice().as_ptr(), pointer);
        let retained = batch.slice(1..3);
        let empty = batch.slice(4..4);
        assert_eq!(retained.allocation_bytes(), capacity);
        assert_eq!(empty.memory_size(), batch.memory_size());
        drop(batch);
        assert_eq!(retained.as_slice(), &[2, 3]);
        assert_eq!(retained.as_slice().as_ptr(), pointer.wrapping_add(1));
    }

    #[test]
    fn slice_accounting() {
        let mut payload = Vec::with_capacity(4096);
        payload.push(7u8);
        let batch = StorageBatch::new(vec![payload, vec![8u8]]);
        let slice = batch.slice(1..2);
        assert_eq!(slice.memory_size(), batch.memory_size());
        assert!(slice.memory_size() >= 4096);
    }

    fn storage() -> crate::StorageManager {
        use crate::test_backend::{Data, Recovery, history, storage_settings};
        use std::sync::Arc;
        crate::StorageManager::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            storage_settings(),
        )
        .unwrap()
    }

    fn pending(storage: &crate::StorageManager) -> std::sync::Arc<crate::manager::Pending> {
        use crate::{Identifier, PayloadEncoding, SchemaKey, StoredRecord};
        let record = StoredRecord {
            position: storage.position(1),
            schema: SchemaKey {
                id: Identifier::new("record").unwrap(),
                version: std::num::NonZeroU32::MIN,
                payload_encoding: PayloadEncoding::PostcardV1,
            },
            scope: None,
            item: Identifier::new("item").unwrap(),
            source: None,
            source_time: None,
            payload: vec![7; 32],
        };
        std::sync::Arc::new(crate::manager::Pending::new(
            &storage.inner,
            None,
            1,
            4096,
            crate::manager::Operation::Records(StorageBatch::new(vec![record])),
        ))
    }

    #[test]
    fn pending_retention() {
        let storage = storage();
        let pending = pending(&storage);
        let crate::manager::Operation::Records(values) = &pending.operation else { unreachable!() };
        let detached = values.clone();
        let empty = detached.slice(1..1);
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.queue.push_back(std::sync::Arc::clone(&pending));
            state.collect();
            assert_eq!(state.status.pending_bytes, 4096);
            assert_eq!(state.retained_write_bytes.load(std::sync::atomic::Ordering::SeqCst), 0);
            state.queue.retire_data(1, 1);
            state.collect();
            assert_eq!(state.status.pending_bytes, 4096);
        }
        drop(pending);
        drop(detached);
        assert_eq!(storage.status().pending_bytes, 4096);
        drop(empty);
        assert_eq!(storage.status().pending_bytes, 0);
    }

    #[test]
    fn failure_retention() {
        let storage = storage();
        let pending = pending(&storage);
        let crate::manager::Operation::Records(values) = &pending.operation else { unreachable!() };
        let detached = values.clone();
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.queue.push_back(std::sync::Arc::clone(&pending));
            state.status.failure = Some(std::sync::Arc::new(crate::Error::NotReady { domain: "injected backend" }));
            state.status.closing = true;
            state.collect();
            assert_eq!(state.status.pending_bytes, 4096);
            assert_eq!(state.retained_write_bytes.load(std::sync::atomic::Ordering::SeqCst), 0);
            state.queue.retire_data(1, 1);
            state.status.closed = true;
        }
        drop(pending);
        assert_eq!(storage.status().pending_bytes, 4096);
        drop(detached);
        assert_eq!(storage.status().pending_bytes, 0);
    }

    #[test]
    fn payload_merge() {
        let left = vec![7u8; 32];
        let right = vec![8u8; 32];
        let pointers = [left.as_ptr(), right.as_ptr()];
        let mut first = StorageBatch::new(vec![left]);
        let mut second = StorageBatch::new(vec![right]);
        let merged = StorageBatch::merge(&mut [&mut first, &mut second]).unwrap();
        assert_eq!(merged.as_slice()[0].as_ptr(), pointers[0]);
        assert_eq!(merged.as_slice()[1].as_ptr(), pointers[1]);
        let detached = merged.clone();
        let retry = StorageBatch::merge(&mut [&mut first, &mut second]).unwrap();
        assert_eq!(retry.len(), 2);
        assert_eq!(retry.as_slice()[1].as_ptr(), pointers[1]);
        drop(detached);
    }

    #[test]
    fn shared_merge_refused() {
        let mut first = StorageBatch::new(vec![vec![7u8; 32]]);
        let mut second = StorageBatch::new(vec![vec![8u8; 32]]);
        let detached = first.clone();
        assert!(StorageBatch::merge(&mut [&mut first, &mut second]).is_none());
        assert_eq!(first.as_slice(), detached.as_slice());
        assert_eq!(second.as_slice()[0], vec![8; 32]);
    }
}
