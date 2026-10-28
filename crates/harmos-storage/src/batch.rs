/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::ops::Range;
use std::sync::Arc;

pub type FileChunk = StorageBatch<u8>;

#[derive(Debug)]
pub struct StorageBatch<T> {
    values: Arc<Vec<T>>,
    range: Range<usize>,
}

impl<T> StorageBatch<T> {
    pub fn new(values: Vec<T>) -> Self {
        let range = 0..values.len();
        Self { values: Arc::new(values), range }
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
            return Some(Self { values: Arc::clone(&first.values), range });
        }
        if batches.iter().any(|batch| Arc::strong_count(&batch.values) != 1 || batch.range != (0..batch.values.len())) {
            return None;
        }
        let count = batches.iter().map(|batch| batch.len()).sum();
        let lengths: Vec<_> = batches.iter().map(|batch| batch.len()).collect();
        let mut values = Vec::with_capacity(count);
        for batch in batches.iter_mut() {
            let source = std::mem::take(Arc::get_mut(&mut batch.values).expect("unique queued backing"));
            values.extend(source);
        }
        let merged = Self {
            values: Arc::new(values),
            range: 0..count,
        };
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
        Self { values: Arc::clone(&self.values), range }
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileChunk, StorageBatch};

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
        assert!(empty.is_empty());
        drop(batch);
        assert_eq!(retained.as_slice(), &[2, 3]);
        assert_eq!(retained.as_slice().as_ptr(), pointer.wrapping_add(1));
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
