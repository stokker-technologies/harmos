/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;

use crate::MemorySize;

/// A retained collection of original typed row pages.
pub type ReadPages<T> = ReadBatch<Vec<ReadBatch<Vec<T>>>>;

pub struct ReadBatch<T> {
    inner: Arc<ReadOwner<T>>,
}

struct ReadOwner<T> {
    value: T,
}

impl<T> ReadBatch<T> {
    pub fn new(value: T) -> Self {
        Self { inner: Arc::new(ReadOwner { value }) }
    }

    /// Moves A Unique Batch Through A Fallible Projection.
    #[tracing::instrument(name = "storage.read_batch.try_map", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn try_map<U, E>(self, map: impl FnOnce(T) -> std::result::Result<U, E>) -> std::result::Result<ReadBatch<U>, E>
    where
        E: From<crate::Error>,
    {
        let owner = Arc::try_unwrap(self.inner).map_err(|_| {
            E::from(crate::Error::InvalidInput {
                field: "read projection",
                reason: "requires a unique owner",
            })
        })?;
        let value = map(owner.value)?;
        tracing::Span::current().record("outcome", "completed");
        Ok(ReadBatch::new(value))
    }

    pub fn try_map_with<U, V, E>(
        self, other: ReadBatch<U>, map: impl FnOnce(T, U) -> std::result::Result<V, E>,
    ) -> std::result::Result<ReadBatch<V>, E>
    where
        E: From<crate::Error>,
    {
        let shared = || {
            E::from(crate::Error::InvalidInput {
                field: "read projection",
                reason: "requires unique owners",
            })
        };
        let left = Arc::try_unwrap(self.inner).map_err(|_| shared())?;
        let right = Arc::try_unwrap(other.inner).map_err(|_| shared())?;
        Ok(ReadBatch::new(map(left.value, right.value)?))
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
            .saturating_add(self.inner.value.memory_size().saturating_sub(std::mem::size_of::<T>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_projection_preserves_payload() {
        let bytes = vec![1u8, 2, 3];
        let pointer = bytes.as_ptr();
        let batch = ReadBatch::new(bytes);
        let alias = batch.clone();
        assert!(batch.try_map(Ok::<_, crate::Error>).is_err());
        assert_eq!(alias.as_ptr(), pointer);
        let projected = alias.try_map(|bytes| Ok::<_, crate::Error>(vec![bytes])).unwrap();
        assert_eq!(projected[0].as_ptr(), pointer);
        assert_eq!(projected[0], [1, 2, 3]);
    }
}
