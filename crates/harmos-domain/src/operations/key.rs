/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub const MAX_CACHE_KEY_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey(Box<[u8]>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CacheKeyError {
    #[error("cache key has {actual} bytes; maximum is {maximum}")]
    TooLarge { actual: usize, maximum: usize },
}

impl CacheKey {
    pub fn new(bytes: impl Into<Box<[u8]>>) -> Result<Self, CacheKeyError> {
        let bytes = bytes.into();
        if bytes.len() > MAX_CACHE_KEY_BYTES {
            return Err(CacheKeyError::TooLarge {
                actual: bytes.len(),
                maximum: MAX_CACHE_KEY_BYTES,
            });
        }

        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
