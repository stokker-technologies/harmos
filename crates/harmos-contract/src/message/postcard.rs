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

// Third Party Imports

// Local Imports
use super::{MessageError, decoding_add, decoding_multiply, decoding_vector};

/* === Constants === */
const MAX_DEPTH: usize = 32;
const MAX_VALUES: usize = 1_048_576;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Append<'a>(&'a mut Vec<u8>);

#[derive(Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    depth: usize,
    remaining: usize,
}

/* === Enums === */

/* === Traits === */

pub trait Value: Sized + Send + Sync {
    fn write(&self, bytes: &mut Vec<u8>);
    fn size(&self) -> usize;
    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError>;
    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError>;

    fn size_many(values: &[Self]) -> usize {
        values
            .iter()
            .fold(0usize, |size, value| size.saturating_add(value.size()))
    }
    fn write_many(values: &[Self], bytes: &mut Vec<u8>) {
        for value in values {
            value.write(bytes);
        }
    }
    fn memory_many(input: &mut Reader<'_>, count: usize) -> Result<usize, MessageError> {
        input.claim(count)?;
        let mut bytes = decoding_multiply(count, std::mem::size_of::<Self>())?;
        for _ in 0..count {
            bytes = decoding_add(
                bytes,
                Self::memory(input)?.saturating_sub(std::mem::size_of::<Self>()),
            )?;
        }
        Ok(bytes)
    }
    fn read_many(input: &mut Reader<'_>, count: usize) -> Result<Vec<Self>, MessageError> {
        input.claim(count)?;
        let mut values = decoding_vector(count)?;
        for _ in 0..count {
            values.push(Self::read(input)?);
        }
        Ok(values)
    }
}

/* === Implementations === */
impl Extend<u8> for Append<'_> {
    fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            depth: 0,
            remaining: MAX_VALUES,
        }
    }
    pub fn remaining(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn finish(self) -> Result<(), MessageError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(MessageError::borrowed("trailing Postcard bytes"))
        }
    }
    pub fn value<T: serde::Deserialize<'a>>(&mut self) -> Result<T, MessageError> {
        self.claim(1)?;
        let (value, rest) = ::postcard::take_from_bytes(self.bytes)
            .map_err(|_| MessageError::borrowed("invalid Postcard value"))?;
        self.bytes = rest;
        Ok(value)
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], MessageError> {
        let (value, rest) = self
            .bytes
            .split_at_checked(count)
            .ok_or_else(|| MessageError::borrowed("truncated Postcard bytes"))?;
        self.bytes = rest;
        Ok(value)
    }
    pub fn claim(&mut self, count: usize) -> Result<(), MessageError> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or_else(|| MessageError::borrowed("Postcard structural work limit exceeded"))?;
        Ok(())
    }
    pub fn nested<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, MessageError>,
    ) -> Result<T, MessageError> {
        if self.depth >= MAX_DEPTH {
            return Err(MessageError::borrowed("Postcard nesting limit exceeded"));
        }
        self.claim(1)?;
        self.depth += 1;
        let result = read(self);
        self.depth -= 1;
        result
    }
}

macro_rules! scalar {
    ($($ty:ty),* $(,)?) => {$(
        impl Value for $ty {
            fn write(&self, bytes: &mut Vec<u8>) {
                ::postcard::to_extend(self, Append(bytes)).expect("scalar Postcard encoding is infallible");
            }
            fn size(&self) -> usize {
                ::postcard::experimental::serialized_size(self).expect("scalar Postcard sizing is infallible")
            }
            fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
                let _: Self = input.value()?;
                Ok(std::mem::size_of::<Self>())
            }
            fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> { input.value() }
        }
    )*};
}
scalar!((), bool, u32, u64, i32, i64, f32, f64, usize);

impl Value for u8 {
    fn write(&self, bytes: &mut Vec<u8>) {
        bytes.push(*self);
    }
    fn size(&self) -> usize {
        1
    }
    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        input.take(1)?;
        Ok(1)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        Ok(input.take(1)?[0])
    }
    fn size_many(values: &[Self]) -> usize {
        values.len()
    }
    fn write_many(values: &[Self], bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(values);
    }
    fn memory_many(input: &mut Reader<'_>, count: usize) -> Result<usize, MessageError> {
        input.take(count)?;
        Ok(count)
    }
    fn read_many(input: &mut Reader<'_>, count: usize) -> Result<Vec<Self>, MessageError> {
        Ok(Box::<[u8]>::from(input.take(count)?).into_vec())
    }
}

macro_rules! nonzero {
    ($($ty:ty => $scalar:ty),* $(,)?) => {$(
        impl Value for $ty {
            fn write(&self, bytes: &mut Vec<u8>) { self.get().write(bytes); }
            fn size(&self) -> usize { self.get().size() }
            fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
                Self::read(input)?;
                Ok(std::mem::size_of::<Self>())
            }
            fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
                Self::new(<$scalar as Value>::read(input)?)
                    .ok_or_else(|| MessageError::borrowed("zero is not a nonzero value"))
            }
        }
    )*};
}
nonzero!(std::num::NonZeroU32 => u32, std::num::NonZeroU64 => u64);

impl Value for String {
    fn write(&self, bytes: &mut Vec<u8>) {
        ::postcard::to_extend(self, Append(bytes)).expect("string Postcard encoding is infallible");
    }
    fn size(&self) -> usize {
        self.len().saturating_add(self.len().size())
    }
    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        let text: &str = input.value()?;
        decoding_add(std::mem::size_of::<Self>(), text.len())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        let text: &str = input.value()?;
        Ok(Box::<str>::from(text).into_string())
    }
}
impl<T: Value> Value for Vec<T> {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.len().write(bytes);
        T::write_many(self, bytes);
    }
    fn size(&self) -> usize {
        self.len().size().saturating_add(T::size_many(self))
    }
    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        input.nested(|input| {
            let count: usize = input.value()?;
            decoding_add(std::mem::size_of::<Self>(), T::memory_many(input, count)?)
        })
    }
    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            let count = input.value()?;
            T::read_many(input, count)
        })
    }
}
impl<T: Value> Value for Option<T> {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.is_some().write(bytes);
        if let Some(value) = self {
            value.write(bytes);
        }
    }
    fn size(&self) -> usize {
        1usize.saturating_add(self.as_ref().map_or(0, Value::size))
    }
    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        input.nested(|input| {
            let present: bool = input.value()?;
            let allocation = if present {
                T::memory(input)?.saturating_sub(std::mem::size_of::<T>())
            } else {
                0
            };
            decoding_add(std::mem::size_of::<Self>(), allocation)
        })
    }
    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            if input.value::<bool>()? {
                T::read(input).map(Some)
            } else {
                Ok(None)
            }
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
