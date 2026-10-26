/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod allocation;
mod identity;
pub mod postcard;

pub use identity::{Digest, Fingerprint};

/* === Re-Exports === */

// Codec Internals For This Crate; Generated Code Reaches Them Through `__private`.
pub(crate) use allocation::{add as decoding_add, multiply as decoding_multiply, vector as decoding_vector};
pub(crate) use postcard::{Reader as PostcardReader, Value as PostcardValue};

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::collections::BTreeMap;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

macro_rules! inline_memory {
    ($($ty:ty),* $(,)?) => {$ (
        impl MemorySize for $ty {
            const INLINE_ONLY: bool = true;

            fn memory_size(&self) -> usize {
                std::mem::size_of::<Self>()
            }
        }
    )*};
}

inline_memory!((), bool, u8, u32, u64, i32, i64, f32, f64);

#[macro_export]
macro_rules! postcard_product {
    ($ty:ty { $($field:ident: $field_ty:ty),* $(,)? } $(, validate = $validate:expr)?) => {
        #[allow(unused_variables)]
        impl $crate::__private::PostcardValue for $ty {
            fn write(&self, bytes: &mut ::std::vec::Vec<u8>) {
                $(<$field_ty as $crate::__private::PostcardValue>::write(&self.$field, bytes);)*
            }
            fn size(&self) -> usize {
                0usize$(.saturating_add(<$field_ty as $crate::__private::PostcardValue>::size(&self.$field)))*
            }
            fn memory(input: &mut $crate::__private::PostcardReader<'_>) -> ::std::result::Result<usize, $crate::MessageError> {
                input.nested(|input| {
                    let memory = std::mem::size_of::<Self>();
                    $(let memory = $crate::__private::decoding_add(memory,
                        <$field_ty as $crate::__private::PostcardValue>::memory(input)?.saturating_sub(std::mem::size_of::<$field_ty>()))?;)*
                    Ok(memory)
                })
            }
            fn read(input: &mut $crate::__private::PostcardReader<'_>) -> ::std::result::Result<Self, $crate::MessageError> {
                input.nested(|input| {
                    let value = Self { $($field: <$field_ty as $crate::__private::PostcardValue>::read(input)?),* };
                    $(($validate)(&value)?;)?
                    Ok(value)
                })
            }
        }
    };
}
pub(crate) use postcard_product;

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid message: {0}")]
pub struct MessageError(Cow<'static, str>);

/* === Enums === */

/* === Traits === */

pub trait Message: Sized + Send + Sync {
    const CODEC: &'static str;
    fn encode(&self) -> Vec<u8>;
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.encode());
    }
    fn encoded_size(&self) -> usize;
    fn decode(bytes: &[u8]) -> Result<Self, MessageError>;
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError>;
}

pub trait MemorySize: Sized {
    /// True only when every value occupies exactly `size_of::<Self>()` bytes,
    /// with no separately owned allocations or additional accounting charge.
    const INLINE_ONLY: bool = false;

    fn memory_size(&self) -> usize;
}

// A Value That Crosses A Process Boundary: Every Operation Request, Response
// And Stream Item. Transport Codecs Derive From The Postcard Value And Its Shape.
pub trait Portable: Message + MemorySize + crate::schema::Described + Send + Sync + 'static {
    fn fingerprint() -> Fingerprint;
    fn digest(&self) -> Digest;
}

pub trait Storable: Message + MemorySize {}

/* === Implementations === */

impl<T> Portable for T
where
    T: Message + MemorySize + crate::schema::Described + Send + Sync + 'static,
{
    fn fingerprint() -> Fingerprint {
        let schema = crate::schema::graph::Schema::from_declaration(T::DEFINITION, &crate::config::SchemaSettings::default())
            .expect("valid portable schema");
        let value = schema.root().expect("portable schema root");
        crate::schema::identity::fingerprint(value, T::CODEC).expect("valid portable schema identity")
    }

    fn digest(&self) -> Digest {
        use sha2::{Digest as _, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"harmos.content.v1\0");
        hash.update(Self::fingerprint().0);
        hash.update(self.encode());
        Digest(hash.finalize().into())
    }
}

impl<T: PostcardValue> Message for T {
    const CODEC: &'static str = "harmos.postcard.v1";
    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size());
        self.write(&mut bytes);
        bytes
    }
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        self.write(bytes);
    }
    fn encoded_size(&self) -> usize {
        self.size()
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        let mut input = PostcardReader::new(bytes);
        let memory = Self::memory(&mut input)?;
        input.finish()?;
        Ok(memory)
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <Self as Message>::decoding_memory(bytes)?;
        let mut input = PostcardReader::new(bytes);
        let value = Self::read(&mut input)?;
        input.finish()?;
        Ok(value)
    }
}

impl<T: Message + MemorySize> Storable for T {}

impl MemorySize for String {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.capacity())
    }
}

impl<T: MemorySize> MemorySize for Vec<T> {
    fn memory_size(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>().saturating_add(self.capacity().saturating_mul(std::mem::size_of::<T>()));
        if T::INLINE_ONLY {
            return bytes;
        }

        for value in self {
            let owned = value.memory_size();
            if owned == usize::MAX {
                return usize::MAX;
            }
            bytes = bytes.saturating_add(owned.saturating_sub(std::mem::size_of::<T>()));
        }
        bytes
    }
}

impl<T: MemorySize> MemorySize for Option<T> {
    const INLINE_ONLY: bool = T::INLINE_ONLY;

    fn memory_size(&self) -> usize {
        let Some(value) = self else {
            return std::mem::size_of::<Self>();
        };
        let owned = value.memory_size();
        if owned == usize::MAX {
            return usize::MAX;
        }

        std::mem::size_of::<Self>().saturating_add(owned.saturating_sub(std::mem::size_of::<T>()))
    }
}

// The Heap Pointee Owns Its Full Resident Size Beside The Inline Pointer
impl<T: MemorySize> MemorySize for Box<T> {
    fn memory_size(&self) -> usize {
        let owned = (**self).memory_size();
        if owned == usize::MAX {
            return usize::MAX;
        }

        std::mem::size_of::<Self>().saturating_add(owned)
    }
}

impl<K: MemorySize, V: MemorySize> MemorySize for BTreeMap<K, V> {
    fn memory_size(&self) -> usize {
        let mut bytes = btree_map_memory::<K, V>(self.len());
        for (key, value) in self {
            let key_bytes = key.memory_size();
            let value_bytes = value.memory_size();
            if key_bytes == usize::MAX || value_bytes == usize::MAX {
                return usize::MAX;
            }
            bytes = bytes
                .saturating_add(key_bytes.saturating_sub(std::mem::size_of::<K>()))
                .saturating_add(value_bytes.saturating_sub(std::mem::size_of::<V>()));
        }
        bytes
    }
}

impl MessageError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self(Cow::Owned(reason.into()))
    }

    pub const fn borrowed(reason: &'static str) -> Self {
        Self(Cow::Borrowed(reason))
    }
}

/* === Public Functions === */

pub fn btree_map_memory<K, V>(entries: usize) -> usize {
    let alignment = std::mem::align_of::<K>().max(std::mem::align_of::<V>()).max(std::mem::align_of::<usize>());
    let node = std::mem::size_of::<K>()
        .saturating_add(std::mem::size_of::<V>())
        .saturating_mul(11)
        .saturating_add(13usize.saturating_mul(std::mem::size_of::<usize>()))
        .saturating_add(2 * std::mem::size_of::<u16>())
        .saturating_add(5usize.saturating_mul(alignment));
    std::mem::size_of::<BTreeMap<K, V>>().saturating_add(entries.saturating_add(1).saturating_mul(node))
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::MemorySize;

    #[test]
    fn vector_allocations() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static VISITS: AtomicUsize = AtomicUsize::new(0);
        struct Inline(u8);
        impl MemorySize for Inline {
            const INLINE_ONLY: bool = true;

            fn memory_size(&self) -> usize {
                VISITS.fetch_add(1, Ordering::Relaxed);
                std::mem::size_of_val(&self.0)
            }
        }

        let mut values = Vec::with_capacity(128);
        values.push(Some(Inline(7)));
        let allocation = values.capacity() * std::mem::size_of::<Option<Inline>>();
        assert_eq!(values.memory_size(), std::mem::size_of_val(&values) + allocation);
        assert_eq!(VISITS.load(Ordering::Relaxed), 0);

        let mut text = String::with_capacity(256);
        text.push_str("nested");
        let text_allocation = text.capacity();
        let mut nested = Vec::with_capacity(8);
        nested.push(Some(text));
        let allocation = nested.capacity() * std::mem::size_of::<Option<String>>();
        let expected = std::mem::size_of_val(&nested) + allocation + text_allocation;
        assert_eq!(nested.memory_size(), expected);
    }

    #[test]
    fn wrappers_saturate() {
        struct Overflow;
        impl MemorySize for Overflow {
            fn memory_size(&self) -> usize {
                usize::MAX
            }
        }

        assert_eq!(Some(Overflow).memory_size(), usize::MAX);
        assert_eq!(vec![Overflow].memory_size(), usize::MAX);
        assert_eq!(Box::new(Overflow).memory_size(), usize::MAX);
        assert_eq!(None::<Overflow>.memory_size(), std::mem::size_of::<Option<Overflow>>());
    }
}
