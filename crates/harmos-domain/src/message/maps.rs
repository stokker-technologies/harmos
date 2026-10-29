/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Deterministic Standard Maps With Finite Entry Schemas And Bounded Decoding.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, BuildHasherDefault, Hash, RandomState};
use std::marker::PhantomData;
use std::mem::size_of;
// Third Party Imports

// Local Imports
use super::postcard::{Reader, Value};
use super::{MemorySize, MessageError, btree_map_memory, decoding_add};
use crate::schema::declaration::{Definition, Name};
use crate::schema::graph::{Field, Kind};
use crate::schema::{Cardinality, Described};

/* === Constants === */

/* === Macros === */

macro_rules! keys {
    ($($ty:ty),* $(,)?) => { $(impl Key for $ty {})* };
}

keys!(bool, u32, u64, i32, i64, String);

macro_rules! map_schema {
    ($container:ident $(, $hasher:ident)?) => {
        impl<K, V $(, $hasher)?> Described for $container<K, V $(, $hasher)?>
        where
            K: Key,
            V: Described,
            $($hasher: 'static,)?
        {
            const NAME: &'static str = Shape::<K, V>::NAME;
            const FIELDS: &'static [Field<Definition>] = Shape::<K, V>::FIELDS;
            const MARKER: &'static [u8] = Shape::<K, V>::MARKER;
            const DEFINITION: Definition = Shape::<K, V>::DEFINITION;
        }
    };
}

map_schema!(BTreeMap);
map_schema!(HashMap, S);

/* === Type Aliases === */

/* === Structs === */

// Schema-Only Entries Have The Same Bytes As Consecutive Key And Value Payloads.
pub struct Entry<K, V>(PhantomData<(K, V)>);
struct Shape<K, V>(PhantomData<(K, V)>);

/* === Enums === */

/* === Traits === */

// Protobuf-Compatible Keys Also Have A Stable Total Ordering.
pub trait Key: Value + MemorySize + Described + Ord + Hash {}

/* === Implementations === */

impl MemorySize for RandomState {
    const INLINE_ONLY: bool = true;
    fn memory_size(&self) -> usize {
        size_of::<Self>()
    }
}

impl<T> MemorySize for BuildHasherDefault<T> {
    const INLINE_ONLY: bool = true;
    fn memory_size(&self) -> usize {
        size_of::<Self>()
    }
}

impl<K: Key, V: Described> Described for Entry<K, V> {
    const NAME: &'static str = Name::new("harmos::MapEntry", &[K::NAME, V::NAME]).as_str();
    const FIELDS: &'static [Field<Definition>] = &[
        Field {
            name: Cow::Borrowed("key"),
            tag: 1,
            kind: K::FIELD_KIND,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("value"),
            tag: 2,
            kind: V::FIELD_KIND,
            cardinality: Cardinality::Required,
        },
    ];
    const MARKER: &'static [u8] = crate::schema::declaration::encode_buffer::<2048>(Self::NAME, Self::FIELDS)
        .split_at(crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS))
        .0;
}

impl<K: Key, V: Described> Described for Shape<K, V> {
    const NAME: &'static str = Name::new("harmos::Map", &[K::NAME, V::NAME]).as_str();
    const FIELDS: &'static [Field<Definition>] = &[Field {
        name: Cow::Borrowed("entries"),
        tag: 1,
        kind: Kind::Message { definition: Entry::<K, V>::DEFINITION },
        cardinality: Cardinality::Repeated,
    }];
    const MARKER: &'static [u8] = crate::schema::declaration::encode_buffer::<2048>(Self::NAME, Self::FIELDS)
        .split_at(crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS))
        .0;
}

impl<K: Key, V: Value> Value for BTreeMap<K, V> {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.len().write(bytes);
        for (key, value) in self {
            key.write(bytes);
            value.write(bytes);
        }
    }

    fn size(&self) -> usize {
        self.iter().fold(self.len().size(), |size, (key, value)| size.saturating_add(key.size()).saturating_add(value.size()))
    }

    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        memory::<K, V>(input, |count| btree_map_memory::<K, V>(count))
    }

    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            let count = count(input)?;
            let mut values = Self::new();
            for _ in 0..count {
                let (key, value) = input.nested(|input| Ok((K::read(input)?, V::read(input)?)))?;
                if values.insert(key, value).is_some() {
                    return Err(MessageError::borrowed("duplicate map key"));
                }
            }
            Ok(values)
        })
    }
}

impl<K: Key, V: Value, S: BuildHasher + Default + MemorySize + Send + Sync> Value for HashMap<K, V, S> {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.len().write(bytes);
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_unstable_by_key(|(key, _)| *key);
        for (key, value) in entries {
            key.write(bytes);
            value.write(bytes);
        }
    }

    fn size(&self) -> usize {
        self.iter().fold(self.len().size(), |size, (key, value)| size.saturating_add(key.size()).saturating_add(value.size()))
    }

    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        memory::<K, V>(input, |count| {
            size_of::<Self>()
                .saturating_add(table_memory::<K, V>(count))
                .saturating_add(S::default().memory_size().saturating_sub(size_of::<S>()))
        })
    }

    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            let count = count(input)?;
            let mut values = Self::with_hasher(S::default());
            values.try_reserve(count).map_err(|_| MessageError::borrowed("map allocation exceeds its addressable capacity"))?;
            for _ in 0..count {
                let (key, value) = input.nested(|input| Ok((K::read(input)?, V::read(input)?)))?;
                if values.insert(key, value).is_some() {
                    return Err(MessageError::borrowed("duplicate map key"));
                }
            }
            Ok(values)
        })
    }
}

impl<K: MemorySize, V: MemorySize, S: MemorySize> MemorySize for HashMap<K, V, S> {
    fn memory_size(&self) -> usize {
        let hasher = self.hasher().memory_size();
        if hasher == usize::MAX {
            return usize::MAX;
        }
        let mut bytes =
            size_of::<Self>().saturating_add(table_memory::<K, V>(self.capacity())).saturating_add(hasher.saturating_sub(size_of::<S>()));
        for (key, value) in self {
            let key_bytes = key.memory_size();
            let value_bytes = value.memory_size();
            if key_bytes == usize::MAX || value_bytes == usize::MAX {
                return usize::MAX;
            }
            bytes =
                bytes.saturating_add(key_bytes.saturating_sub(size_of::<K>())).saturating_add(value_bytes.saturating_sub(size_of::<V>()));
        }
        bytes
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn count(input: &mut Reader<'_>) -> Result<usize, MessageError> {
    let count: usize = input.value()?;
    input.claim(count)?;
    Ok(count)
}

fn memory<K: Value, V: Value>(input: &mut Reader<'_>, allocation: impl FnOnce(usize) -> usize) -> Result<usize, MessageError> {
    input.nested(|input| {
        let count = count(input)?;
        let mut bytes = allocation(count);
        if bytes == usize::MAX {
            return Err(MessageError::borrowed("map allocation exceeds its addressable capacity"));
        }
        for _ in 0..count {
            let owned = input.nested(|input| {
                let key = K::memory(input)?.saturating_sub(size_of::<K>());
                let value = V::memory(input)?.saturating_sub(size_of::<V>());
                decoding_add(key, value)
            })?;
            bytes = decoding_add(bytes, owned)?;
        }
        Ok(bytes)
    })
}

// Charge Bucket Padding, The Control Group And A Conservative 7/8 Load Factor.
fn table_memory<K, V>(capacity: usize) -> usize {
    if capacity == 0 {
        return 0;
    }
    let buckets = capacity
        .checked_mul(8)
        .and_then(|value| value.checked_add(6))
        .map(|value| value / 7)
        .and_then(|value| value.max(4).checked_next_power_of_two());
    buckets
        .and_then(|buckets| buckets.checked_mul(size_of::<(K, V)>().saturating_add(1)))
        .and_then(|bytes| bytes.checked_add(16 + std::mem::align_of::<(K, V)>()))
        .unwrap_or(usize::MAX)
}

/* === Tests === */
