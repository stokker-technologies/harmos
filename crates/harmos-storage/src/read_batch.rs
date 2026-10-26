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
use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::MemorySize;
use crate::arrow::RetainedMemory;
use crate::capacity::Reservation;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/// A retained collection of original typed row pages.
pub type ReadPages<T> = ReadBatch<Vec<ReadBatch<Vec<T>>>>;

/* === Structs === */

pub struct ReadBatch<T> {
    inner: Arc<ReadOwner<T>>,
}

struct ReadOwner<T> {
    value: T,
    _memory: Vec<RetainedMemory>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> crate::StorageManager<C> {
    #[tracing::instrument(name = "storage.read_batch.read_memory", level = "trace", skip_all, fields(bytes = bytes.get(), outcome = "cancelled"))]
    pub async fn read_memory(&self, bytes: std::num::NonZeroUsize) -> crate::Result<RetainedMemory> {
        let result = async {
            crate::capacity::Reservation::wait_lifecycle(
                &self.inner,
                crate::capacity::Pool::Read,
                &crate::capacity::Held::default(),
                bytes.get(),
            )
            .await
            .map(Reservation::retain)
            .map_err(crate::capacity::Refusal::into_error)
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }
}

impl<T> ReadBatch<T> {
    pub(crate) fn new(value: T, reservation: Reservation) -> Self {
        Self::retained(value, reservation.retain())
    }

    pub(crate) fn retained(value: T, memory: RetainedMemory) -> Self {
        Self { inner: Arc::new(ReadOwner { value, _memory: vec![memory] }) }
    }

    /// Consumes a unique owner and preserves its admitted memory through a projection.
    ///
    /// `additional_peak` must bound all new output allocations and conversion scratch
    /// simultaneously live with the input. Private owner growth is admitted separately.
    /// The callback must transfer payload allocations only into its successful value.
    /// Errors and external captures must not retain them. Nested retained pages must
    /// retain or map their own owners.
    ///
    /// Admission is nonblocking and happens before the callback. Shared owners are
    /// refused; failure consumes this handle. Existing and additional claims remain
    /// conservative until their last owner drops, including on independently shared
    /// memory claims. No automatic shrinking or payload cloning occurs.
    #[tracing::instrument(name = "storage.read_batch.try_map", level = "trace", skip_all, fields(additional_peak = additional_peak, outcome = "cancelled"))]
    pub fn try_map<U, E>(
        self, additional_peak: usize, map: impl FnOnce(T) -> std::result::Result<U, E>,
    ) -> std::result::Result<ReadBatch<U>, E>
    where
        E: From<crate::Error>,
    {
        let result: std::result::Result<crate::ReadBatch<U>, E> = (|| {
            let shared = || crate::Error::InvalidInput { field: "read projection", reason: "requires a unique owner" };
            // Weak aliases can retain the old Arc allocation after its value moves.
            let mut inner = self.inner;
            if Arc::get_mut(&mut inner).is_none() {
                return Err(shared().into());
            }
            let owner = Arc::try_unwrap(inner).map_err(|_| E::from(shared()))?;
            let growth = std::mem::size_of::<ReadOwner<U>>().saturating_sub(std::mem::size_of::<ReadOwner<T>>());
            let additional = additional_peak
                .checked_add(growth)
                .ok_or_else(|| E::from(crate::Error::ResourceExhausted { resource: "read projection memory" }))?;
            if additional != 0 {
                owner._memory.first().expect("read owner retains admission").try_grow_by(additional).map_err(E::from)?;
            }
            let ReadOwner { value, _memory } = owner;
            let value = map(value)?;
            Ok(ReadBatch { inner: Arc::new(ReadOwner { value, _memory }) })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    /// Consumes two unique owners while retaining every input memory claim.
    ///
    /// The callback has the same peak-allocation and payload-escape obligations as
    /// `try_map`. Guard-vector replacement and private owner growth are admitted
    /// separately before allocating. Admission never waits and failure consumes
    /// both handles. Shared or weak owner aliases are refused before the callback.
    ///
    /// Combining claims does not unify their admission groups. Inputs used by later
    /// dependent reads should originate from the same `ReadSession`. Nested pages
    /// must continue to retain or map their own guards.
    #[tracing::instrument(name = "storage.read_batch.try_map_with", level = "trace", skip_all, fields(additional_peak = additional_peak, outcome = "cancelled"))]
    pub fn try_map_with<U, V, E>(
        self, other: ReadBatch<U>, additional_peak: usize, map: impl FnOnce(T, U) -> std::result::Result<V, E>,
    ) -> std::result::Result<ReadBatch<V>, E>
    where
        E: From<crate::Error>,
    {
        let result: std::result::Result<crate::ReadBatch<V>, E> = (|| {
            let shared = || crate::Error::InvalidInput { field: "read projection", reason: "requires unique owners" };
            let exhausted = || crate::Error::ResourceExhausted { resource: "read projection memory" };
            let mut left = self.inner;
            let mut right = other.inner;
            if Arc::get_mut(&mut left).is_none() || Arc::get_mut(&mut right).is_none() {
                return Err(shared().into());
            }
            let mut left = Arc::try_unwrap(left).map_err(|_| E::from(shared()))?;
            let mut right = Arc::try_unwrap(right).map_err(|_| E::from(shared()))?;
            let count = left._memory.len().checked_add(right._memory.len()).ok_or_else(|| E::from(exhausted()))?;
            // A Replacement Lives Alongside Both Original Guard Arrays.
            let target = if count > left._memory.capacity() {
                left._memory.capacity().checked_mul(2).unwrap_or(count).max(count)
            } else {
                left._memory.capacity()
            };
            let metadata = if target > left._memory.capacity() {
                target.checked_mul(std::mem::size_of::<RetainedMemory>()).ok_or_else(|| E::from(exhausted()))?
            } else {
                0
            };
            let growth = std::mem::size_of::<ReadOwner<V>>().saturating_sub(std::mem::size_of::<ReadOwner<T>>());
            let additional =
                additional_peak.checked_add(metadata).and_then(|bytes| bytes.checked_add(growth)).ok_or_else(|| E::from(exhausted()))?;
            if additional != 0 {
                left._memory.first().expect("read owner retains admission").try_grow_by(additional).map_err(E::from)?;
            }
            if target > left._memory.capacity() {
                left._memory.try_reserve_exact(target - left._memory.len()).map_err(|_| E::from(exhausted()))?;
            }
            left._memory.append(&mut right._memory);
            let ReadOwner { value: left, _memory } = left;
            let ReadOwner { value: right, _memory: empty } = right;
            drop(empty);
            let value = map(left, right)?;
            Ok(ReadBatch { inner: Arc::new(ReadOwner { value, _memory }) })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(crate) fn owner_bytes() -> crate::Result<usize> {
        std::mem::size_of::<ReadOwner<T>>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<RetainedMemory>()))
            .ok_or(crate::Error::ResourceExhausted { resource: "read projection memory" })
    }

    pub fn owner(&self) -> Arc<impl Borrow<T> + Send + Sync + 'static>
    where
        T: Send + Sync + 'static,
    {
        Arc::clone(&self.inner)
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ReadBatch<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("ReadBatch").field(&self.inner.value).finish()
    }
}

impl<T> Clone for ReadBatch<T> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<T> Deref for ReadBatch<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner.value
    }
}

impl<T> AsRef<T> for ReadBatch<T> {
    fn as_ref(&self) -> &T {
        &self.inner.value
    }
}

impl<T> Borrow<T> for ReadOwner<T> {
    fn borrow(&self) -> &T {
        &self.value
    }
}

impl<T: MemorySize> MemorySize for ReadBatch<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(std::mem::size_of::<ReadOwner<T>>())
            .saturating_add(2 * std::mem::size_of::<usize>())
            .saturating_add(self.inner._memory.capacity().saturating_mul(std::mem::size_of::<RetainedMemory>()))
            .saturating_add(self.inner.value.memory_size().saturating_sub(std::mem::size_of::<T>()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity::{Held, Pool};
    use crate::test_backend::{Data, Recovery, history, storage_settings};

    fn storage() -> crate::StorageManager {
        let storage = crate::StorageManager::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            storage_settings(),
        )
        .unwrap();
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.ready = true;
            state.status.running = true;
        }
        storage
    }

    #[test]
    fn owner_retention() {
        struct Payload(Vec<u8>);
        let storage = storage();
        let reservation = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap();
        let values = vec![7; 2048];
        let pointer = values.as_ptr();
        let batch = ReadBatch::new(Payload(values), reservation);
        let alias = batch.clone();
        let owner = alias.owner();
        assert_eq!(batch.0.as_ptr(), pointer);
        drop(batch);
        drop(alias);
        assert_eq!(storage.status().read_bytes, 4096);
        let value: &Payload = owner.as_ref().borrow();
        assert_eq!(value.0.as_ptr(), pointer);
        drop(owner);
        assert_eq!(storage.status().read_bytes, 0);
    }
    #[test]
    fn projection_moves() {
        let storage = storage();
        let bytes = storage.inner.settings.budget.read_memory_bytes.get();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), bytes).unwrap();
        let payload = vec![7u8; 32];
        let pointer = payload.as_ptr();
        let batch = ReadBatch::new((payload, 42u64), memory);
        let mapped = batch.try_map(0, |(value, _)| Ok::<_, crate::Error>(value)).unwrap();
        assert_eq!(mapped.as_ptr(), pointer);
        assert_eq!(storage.status().read_bytes, bytes);
        drop(mapped);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_retains() {
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap().retain();
        let alias = memory.clone();
        let batch = ReadBatch::retained(vec![7u8; 32], memory);
        let mapped = batch.try_map(256, Ok::<_, crate::Error>).unwrap();
        assert_eq!(alias.bytes(), 4352);
        let peer = mapped.clone();
        drop(mapped);
        assert_eq!(storage.status().read_bytes, 4352);
        drop(peer);
        assert_eq!(storage.status().read_bytes, 4352);
        drop(alias);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_refuses() {
        let storage = storage();
        let bytes = storage.inner.settings.budget.read_memory_bytes.get();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), bytes).unwrap();
        let batch = ReadBatch::new(vec![7u8; 32], memory);
        let called = std::cell::Cell::new(false);
        let result = batch.try_map(1, |value| {
            called.set(true);
            Ok::<_, crate::Error>(value)
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn shared_projection_refuses() {
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap();
        let batch = ReadBatch::new(vec![7u8; 32], memory);
        let peer = batch.clone();
        let called = std::cell::Cell::new(false);
        let result = batch.try_map(0, |value| {
            called.set(true);
            Ok::<_, crate::Error>(value)
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert_eq!(peer.as_slice(), &[7u8; 32]);
        assert_eq!(storage.status().read_bytes, 4096);
        drop(peer);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_failure_releases() {
        #[derive(Debug)]
        enum Failure {
            Storage,
            Conversion,
        }
        impl From<crate::Error> for Failure {
            fn from(_: crate::Error) -> Self {
                Self::Storage
            }
        }
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap();
        let batch = ReadBatch::new(vec![7u8; 32], memory);
        let result = batch.try_map(256, |_| Err::<Vec<u8>, _>(Failure::Conversion));
        assert!(matches!(result, Err(Failure::Conversion)));
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_owner_growth() {
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap();
        let batch = ReadBatch::new(7u8, memory);
        let mapped = batch.try_map(0, |value| Ok::<_, crate::Error>([value; 128])).unwrap();
        let growth = std::mem::size_of::<ReadOwner<[u8; 128]>>() - std::mem::size_of::<ReadOwner<u8>>();
        assert_eq!(storage.status().read_bytes, 4096 + growth);
        assert_eq!(mapped.as_slice(), &[7u8; 128]);
        drop(mapped);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn incremental_growth() {
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap().retain();
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let memory = memory.clone();
                scope.spawn(move || {
                    for _ in 0..64 {
                        memory.try_grow_by(1).unwrap();
                    }
                });
            }
        });
        assert_eq!(memory.bytes(), 4224);
        assert_eq!(storage.status().read_bytes, 4224);
        drop(memory);
        assert_eq!(storage.status().read_bytes, 0);
    }
    #[test]
    fn weak_projection_refuses() {
        let storage = storage();
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), 4096).unwrap();
        let batch = ReadBatch::new(vec![7u8; 32], memory);
        let weak = Arc::downgrade(&batch.inner);
        let called = std::cell::Cell::new(false);
        let result = batch.try_map(0, |value| {
            called.set(true);
            Ok::<_, crate::Error>(value)
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert!(weak.upgrade().is_none());
        assert_eq!(storage.status().read_bytes, 0);
    }
    fn admitted<T>(storage: &crate::StorageManager, value: T, bytes: usize) -> ReadBatch<T> {
        ReadBatch::new(value, Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), bytes).unwrap())
    }

    #[test]
    fn merge_retains_inputs() {
        let storage = storage();
        let left = admitted(&storage, vec![7u8; 32], 4096);
        let right = admitted(&storage, String::from("retained"), 4096);
        let pointers = (left.as_ptr(), right.as_ptr());
        let first = left.inner._memory[0].clone();
        let second = right.inner._memory[0].clone();
        let merged = left.try_map_with(right, 64, |left, right| Ok::<_, crate::Error>((left, right))).unwrap();
        assert_eq!((merged.0.as_ptr(), merged.1.as_ptr()), pointers);
        let held = first.bytes() + second.bytes();
        assert!(held > 8192);
        assert_eq!(storage.status().read_bytes, held);
        let peer = merged.clone();
        drop(merged);
        drop(peer);
        assert_eq!(storage.status().read_bytes, held);
        drop(first);
        assert_eq!(storage.status().read_bytes, second.bytes());
        drop(second);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn merge_shared_refuses() {
        for share_right in [false, true] {
            let storage = storage();
            let left = admitted(&storage, vec![7u8; 32], 4096);
            let right = admitted(&storage, vec![8u8; 32], 4096);
            let peer = if share_right { right.clone() } else { left.clone() };
            let called = std::cell::Cell::new(false);
            let result = left.try_map_with(right, 0, |left, right| {
                called.set(true);
                Ok::<_, crate::Error>((left, right))
            });
            assert!(result.is_err());
            assert!(!called.get());
            assert_eq!(peer.len(), 32);
            assert_eq!(storage.status().read_bytes, 4096);
            drop(peer);
            assert_eq!(storage.status().read_bytes, 0);
        }
    }

    #[test]
    fn merge_metadata_refuses() {
        let storage = storage();
        let budget = storage.inner.settings.budget.read_memory_bytes.get();
        let left = admitted(&storage, vec![7u8; 32], budget / 2);
        let right = admitted(&storage, vec![8u8; 32], budget - budget / 2);
        let called = std::cell::Cell::new(false);
        let result = left.try_map_with(right, 0, |left, _| {
            called.set(true);
            Ok::<_, crate::Error>(left)
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn merge_overflow_refuses() {
        let storage = storage();
        let left = admitted(&storage, vec![7u8; 32], 4096);
        let right = admitted(&storage, vec![8u8; 32], 4096);
        let result = left.try_map_with(right, usize::MAX, |_, _| -> crate::Result<()> {
            panic!("overflow must refuse before conversion");
        });
        assert!(matches!(result, Err(crate::Error::ResourceExhausted { .. })));
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn merge_failure_releases() {
        let storage = storage();
        let left = admitted(&storage, vec![7u8; 32], 4096);
        let right = admitted(&storage, String::from("retained"), 4096);
        let result = left.try_map_with(right, 64, |_, _| -> crate::Result<()> {
            Err(crate::Error::InvalidInput { field: "projection", reason: "injected conversion failure" })
        });
        assert!(matches!(result, Err(crate::Error::InvalidInput { .. })));
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn merge_folds_pages() {
        let storage = storage();
        let first = vec![7u8; 32];
        let mut pointers = vec![first.as_ptr()];
        let mut merged = admitted(&storage, vec![first], 4096);
        for byte in [8u8, 9] {
            let value = vec![byte; 32];
            pointers.push(value.as_ptr());
            let page = admitted(&storage, value, 4096);
            merged = merged
                .try_map_with(page, 4 * std::mem::size_of::<Vec<u8>>(), |mut values, value| {
                    values.push(value);
                    Ok::<_, crate::Error>(values)
                })
                .unwrap();
        }
        assert_eq!(merged.iter().map(|value| value.as_ptr()).collect::<Vec<_>>(), pointers);
        assert!(storage.status().read_bytes >= 3 * 4096);
        let peer = merged.clone();
        drop(merged);
        assert_eq!(peer.len(), 3);
        drop(peer);
        assert_eq!(storage.status().read_bytes, 0);
    }
    #[test]
    fn merge_weak_refuses() {
        let storage = storage();
        let left = admitted(&storage, vec![7u8; 32], 4096);
        let right = admitted(&storage, vec![8u8; 32], 4096);
        let weak = Arc::downgrade(&right.inner);
        let called = std::cell::Cell::new(false);
        let result = left.try_map_with(right, 0, |left, right| {
            called.set(true);
            Ok::<_, crate::Error>((left, right))
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert!(weak.upgrade().is_none());
        assert_eq!(storage.status().read_bytes, 0);
    }
    #[test]
    fn merge_metadata_bounded() {
        let storage = storage();
        let count = 128usize;
        let mut merged = admitted(&storage, vec![vec![7u8; 32]], 4096);
        for _ in 1..count {
            let page = admitted(&storage, vec![8u8; 32], 4096);
            let peak = if merged.len() == merged.capacity() {
                merged.capacity().checked_mul(2).unwrap().max(4) * std::mem::size_of::<Vec<u8>>()
            } else {
                0
            };
            merged = merged
                .try_map_with(page, peak, |mut values, value| {
                    values.push(value);
                    Ok::<_, crate::Error>(values)
                })
                .unwrap();
        }
        assert_eq!(merged.len(), count);
        let source = count * 4096;
        let scaffold = count * 8 * (std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<RetainedMemory>());
        assert!(storage.status().read_bytes >= source);
        assert!(storage.status().read_bytes - source <= scaffold);
        drop(merged);
        assert_eq!(storage.status().read_bytes, 0);
    }
}
