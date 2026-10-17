/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod input;
pub mod postcard;

/* === Re-Exports === */
// Codec Internals For This Crate; Generated Code Reaches Them Through `__private`.
#[cfg_attr(not(feature = "grpc"), allow(unused_imports))]
pub(crate) use input::{
    DecodeField, DecodeInput, DecodeScalar, DecodeValue, add as decoding_add,
    multiply as decoding_multiply, vector as decoding_vector,
};
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
            fn write(&self, bytes: &mut Vec<u8>) {
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
    fn encode_to_vec(&self) -> Vec<u8>;
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.encode_to_vec());
    }
    fn encoded_len(&self) -> usize;
    fn decode(bytes: &[u8]) -> Result<Self, MessageError>;
    fn decoding_memory(_bytes: &[u8]) -> Result<usize, MessageError> {
        Err(MessageError::borrowed(
            "decoding memory bound is unsupported",
        ))
    }
}

#[cfg(feature = "grpc")]
pub trait Protobuf: Sized + Send + Sync {
    fn encode_to_vec(&self) -> Vec<u8>;
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.encode_to_vec());
    }
    fn encoded_len(&self) -> usize;
    fn decode(bytes: &[u8]) -> Result<Self, MessageError>;
    fn decoding_memory(_bytes: &[u8]) -> Result<usize, MessageError> {
        Err(MessageError::borrowed(
            "decoding memory bound is unsupported",
        ))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> Result<usize, MessageError> {
        Self::decoding_memory(input.contiguous()?)
    }

    fn decode_input(input: DecodeInput<'_>) -> Result<Self, MessageError> {
        Self::decode(input.contiguous()?)
    }
}

pub trait MemorySize: Sized {
    fn memory_size(&self) -> usize;
}

// A Value That Crosses A Process Boundary: Every Operation Request, Response
// And Stream Item. The Protobuf Codec Joins It Exactly When This Crate Carries
// One, So Every Expansion Gated By `with_protobuf!` Agrees With The Bound.
#[cfg(feature = "grpc")]
pub trait Portable:
    Message + Protobuf + MemorySize + crate::schema::Described + Send + Sync + 'static
{
}

#[cfg(not(feature = "grpc"))]
pub trait Portable:
    Message + MemorySize + crate::schema::Described + Send + Sync + 'static
{
}

pub trait Storable: Message + MemorySize {
    fn encoded_size(&self) -> usize {
        self.encoded_len()
    }
}

/* === Implementations === */
#[cfg(feature = "grpc")]
impl<T> Portable for T where
    T: Message + Protobuf + MemorySize + crate::schema::Described + Send + Sync + 'static
{
}

#[cfg(not(feature = "grpc"))]
impl<T> Portable for T where
    T: Message + MemorySize + crate::schema::Described + Send + Sync + 'static
{
}

impl<T: PostcardValue> Message for T {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size());
        self.write(&mut bytes);
        bytes
    }
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        self.write(bytes);
    }
    fn encoded_len(&self) -> usize {
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
        let mut bytes = std::mem::size_of::<Self>()
            .saturating_add(self.capacity().saturating_mul(std::mem::size_of::<T>()));

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

#[cfg(feature = "grpc")]
impl<T: prost::Message + Default + 'static> Protobuf for T {
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        <T as prost::Message>::encode_raw(self, bytes);
    }
    fn encode_to_vec(&self) -> Vec<u8> {
        prost::Message::encode_to_vec(self)
    }
    fn encoded_len(&self) -> usize {
        prost::Message::encoded_len(self)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        primitive_protobuf_memory::<T>(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <T as prost::Message>::decode(bytes).map_err(|error| MessageError::new(error.to_string()))
    }
}

#[cfg(feature = "grpc")]
fn primitive_protobuf_memory<T: 'static>(bytes: &[u8]) -> Result<usize, MessageError> {
    use std::any::TypeId;
    let ty = TypeId::of::<T>();
    let empty = ty == TypeId::of::<()>();
    let text = ty == TypeId::of::<String>();
    let blob = ty == TypeId::of::<Vec<u8>>();
    let scalar = if [
        TypeId::of::<bool>(),
        TypeId::of::<u32>(),
        TypeId::of::<u64>(),
        TypeId::of::<i32>(),
        TypeId::of::<i64>(),
    ]
    .contains(&ty)
    {
        Some(DecodeScalar::Varint)
    } else if ty == TypeId::of::<f32>() {
        Some(DecodeScalar::Bits32)
    } else if ty == TypeId::of::<f64>() {
        Some(DecodeScalar::Bits64)
    } else {
        None
    };
    if !empty && !text && !blob && scalar.is_none() {
        return Err(MessageError::borrowed(
            "decoding memory bound is unsupported",
        ));
    }
    let mut largest = 0;
    DecodeInput::new(bytes).fields(&mut |field| {
        if field.tag != 1 || empty {
            return Ok(());
        }
        if let Some(kind) = scalar {
            field.scalar(kind)?;
        }
        if text {
            largest = largest.max(field.text()?.len());
        }
        if blob {
            largest = largest.max(field.bytes()?.len());
        }
        Ok(())
    })?;
    // Prost May Retain A Previous Duplicate's Capacity While Growing Its Buffer.
    // Include Minimum Vec Allocation, Geometric Growth And Overlapping Storage.
    let heap = if largest == 0 {
        0
    } else {
        decoding_multiply(largest.max(8), 4)?
    };
    decoding_add(std::mem::size_of::<T>(), heap)
}

/* === Public Functions === */
pub fn btree_map_memory<K, V>(entries: usize) -> usize {
    let alignment = std::mem::align_of::<K>()
        .max(std::mem::align_of::<V>())
        .max(std::mem::align_of::<usize>());
    let node = std::mem::size_of::<K>()
        .saturating_add(std::mem::size_of::<V>())
        .saturating_mul(11)
        .saturating_add(13usize.saturating_mul(std::mem::size_of::<usize>()))
        .saturating_add(2 * std::mem::size_of::<u16>())
        .saturating_add(5usize.saturating_mul(alignment));
    std::mem::size_of::<BTreeMap<K, V>>()
        .saturating_add(entries.saturating_add(1).saturating_mul(node))
}

/* === Private Functions === */

/* === Tests === */
