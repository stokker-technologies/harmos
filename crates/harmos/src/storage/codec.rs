/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "codec/timestamp.rs"]
mod timestamp;

/* === Re-Exports === */
pub(crate) use timestamp::Time;

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::marker::PhantomData;
use std::num::{NonZeroU32, NonZeroUsize};

// Third Party Imports

// Local Imports
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
#[cfg(feature = "grpc")]
use crate::__private::Protobuf as Message;
use crate::__private::{PostcardReader, PostcardValue};
use crate::schema::{Cardinality, Described, Kind};
#[cfg(feature = "grpc")]
use crate::storage_position::codec::{
    bytes_field, field_len, identifier, text_field, uint_field, write_bytes, write_header,
};
use crate::{Identifier, MemorySize, MessageError};

/* === Constants === */

/* === Macros === */
macro_rules! product {
    ($ty:ty { $($field:ident: $codec:ty => $tag:literal),* $(,)? }) => {
        crate::storage_codec::product!($ty { $($field: $codec => $tag),* }, check = |_: &$ty| Ok(()));
    };
    ($ty:ty { $($field:ident: $codec:ty => $tag:literal),* $(,)? }, check = $check:expr) => {
        impl crate::schema::Described for $ty {
            const FIELDS: &'static [crate::schema::Field] = &[$(crate::schema::Field {
                name: std::borrow::Cow::Borrowed(stringify!($field)),
                tag: $tag,
                kind: <$codec as crate::storage_codec::Value>::KIND,
                cardinality: <$codec as crate::storage_codec::Value>::CARDINALITY,
            }),*];
        }
        impl crate::__private::PostcardValue for $ty {
            fn write(&self, bytes: &mut Vec<u8>) {
                $(<$codec as crate::storage_codec::Value>::postcard_write(&self.$field, bytes);)*
            }
            fn size(&self) -> usize {
                0usize$(.saturating_add(<$codec as crate::storage_codec::Value>::postcard_size(&self.$field)))*
            }
            fn memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, crate::MessageError> {
                input.nested(|input| {
                    let memory = std::mem::size_of::<Self>();
                    $(let memory = crate::__private::decoding_add(memory, <$codec as crate::storage_codec::Value>::postcard_memory(input)?)?;)*
                    Ok(memory)
                })
            }
            fn read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<Self, crate::MessageError> {
                let value = input.nested(|input| Ok(Self { $($field: <$codec as crate::storage_codec::Value>::postcard_read(input)?),* }))?;
                let check: fn(&Self) -> std::result::Result<(), crate::MessageError> = $check;
                check(&value)?;
                Ok(value)
            }
        }
        #[cfg(feature = "grpc")]
        impl crate::__private::Protobuf for $ty {
            fn encode_to_vec(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
                <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
                bytes
            }
            fn encode_into(&self, bytes: &mut Vec<u8>) {
                $(<$codec as crate::storage_codec::Value>::write(&self.$field, $tag, bytes);)*
            }
            fn encoded_len(&self) -> usize {
                0usize$(.saturating_add(<$codec as crate::storage_codec::Value>::size(&self.$field, $tag)))*
            }
            fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
                <Self as crate::__private::Protobuf>::decode_input(crate::__private::DecodeInput::new(bytes))
            }
            fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
                <Self as crate::__private::Protobuf>::decoding_memory_input(crate::__private::DecodeInput::new(bytes))
            }
            fn decoding_memory_input(input: crate::__private::DecodeInput<'_>) -> std::result::Result<usize, crate::MessageError> {
                let memory = std::mem::size_of::<Self>();
                $(let memory = crate::__private::decoding_add(memory, <$codec as crate::storage_codec::Value>::memory(input, $tag)?)?;)*
                Ok(memory)
            }
            fn decode_input(input: crate::__private::DecodeInput<'_>) -> std::result::Result<Self, crate::MessageError> {
                <Self as crate::__private::Protobuf>::decoding_memory_input(input)?;
                let value = Self { $($field: <$codec as crate::storage_codec::Value>::read(input, $tag)?),* };
                let check: fn(&Self) -> std::result::Result<(), crate::MessageError> = $check;
                check(&value)?;
                Ok(value)
            }
        }
    };
}

pub(crate) use product;

macro_rules! postcard_field {
    () => {
        fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
            crate::__private::PostcardValue::write(value, bytes);
        }
        fn postcard_size(value: &Self::Native) -> usize {
            crate::__private::PostcardValue::size(value)
        }
        fn postcard_memory(
            input: &mut crate::__private::PostcardReader<'_>,
        ) -> std::result::Result<usize, crate::MessageError> {
            Ok(
                <Self::Native as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<Self::Native>()),
            )
        }
        fn postcard_read(
            input: &mut crate::__private::PostcardReader<'_>,
        ) -> std::result::Result<Self::Native, crate::MessageError> {
            <Self::Native as crate::__private::PostcardValue>::read(input)
        }
    };
}
pub(crate) use postcard_field;

macro_rules! variants {
    ($ty:ty { $($index:literal, $tag:literal, $name:literal: $pat:pat => { $($field:ident: $codec:ty => $ftag:literal),* $(,)? } => $build:expr;)+ }) => {
        impl crate::schema::Described for $ty {
            const FIELDS: &'static [crate::schema::Field] = &[$(crate::schema::Field {
                name: std::borrow::Cow::Borrowed($name),
                tag: $tag,
                cardinality: crate::schema::Cardinality::Variant,
                kind: crate::schema::Kind::Message {
                    fields: std::borrow::Cow::Borrowed(&[$(crate::schema::Field {
                        name: std::borrow::Cow::Borrowed(stringify!($field)),
                        tag: $ftag,
                        kind: <$codec as crate::storage_codec::Value>::KIND,
                        cardinality: <$codec as crate::storage_codec::Value>::CARDINALITY,
                    }),*]),
                },
            }),+];
        }
        impl crate::__private::PostcardValue for $ty {
            #[allow(unused_variables)]
            fn write(&self, bytes: &mut Vec<u8>) {
                match self {
                    $($pat => {
                        crate::__private::PostcardValue::write(&($index as u32), bytes);
                        $(<$codec as crate::storage_codec::Value>::postcard_write($field, bytes);)*
                    })+
                }
            }
            #[allow(unused_variables)]
            fn size(&self) -> usize {
                match self {
                    $($pat => crate::__private::PostcardValue::size(&($index as u32))
                        $(.saturating_add(<$codec as crate::storage_codec::Value>::postcard_size($field)))*,)+
                }
            }
            fn memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, crate::MessageError> {
                input.nested(|input| {
                    let memory = std::mem::size_of::<Self>();
                    match input.value::<u32>()? {
                        $($index => {
                            $(let memory = crate::__private::decoding_add(memory, <$codec as crate::storage_codec::Value>::postcard_memory(input)?)?;)*
                            Ok(memory)
                        })+
                        _ => Err(crate::MessageError::borrowed(concat!("unknown ", stringify!($ty), " variant"))),
                    }
                })
            }
            fn read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<Self, crate::MessageError> {
                input.nested(|input| match input.value::<u32>()? {
                    $($index => {
                        $(let $field = <$codec as crate::storage_codec::Value>::postcard_read(input)?;)*
                        Ok($build)
                    })+
                    _ => Err(crate::MessageError::borrowed(concat!("unknown ", stringify!($ty), " variant"))),
                })
            }
        }
        crate::with_protobuf! {
            impl crate::__private::Protobuf for $ty {
                fn encode_to_vec(&self) -> Vec<u8> {
                    let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
                    <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
                    bytes
                }
                #[allow(unused_variables)]
                fn encode_into(&self, bytes: &mut Vec<u8>) {
                    match self {
                        $($pat => {
                            let length = 0usize$(.saturating_add(<$codec as crate::storage_codec::Value>::size($field, $ftag)))*;
                            crate::storage_position::codec::write_header($tag, length, bytes);
                            $(<$codec as crate::storage_codec::Value>::write($field, $ftag, bytes);)*
                        })+
                    }
                }
                #[allow(unused_variables)]
                fn encoded_len(&self) -> usize {
                    match self {
                        $($pat => crate::storage_codec::length_delimited_size(
                            $tag,
                            0usize$(.saturating_add(<$codec as crate::storage_codec::Value>::size($field, $ftag)))*,
                        ),)+
                    }
                }
                fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
                    <Self as crate::__private::Protobuf>::decode_input(crate::__private::DecodeInput::new(bytes))
                }
                fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
                    <Self as crate::__private::Protobuf>::decoding_memory_input(crate::__private::DecodeInput::new(bytes))
                }
                #[allow(unused_variables)]
                fn decoding_memory_input(input: crate::__private::DecodeInput<'_>) -> std::result::Result<usize, crate::MessageError> {
                    let (tag, child) = input
                        .variant(&[$($tag),+])?
                        .ok_or_else(|| crate::MessageError::borrowed(concat!("missing ", stringify!($ty), " variant")))?;
                    let memory = std::mem::size_of::<Self>();
                    match tag {
                        $($tag => {
                            child.fields(&mut |_| Ok(()))?;
                            $(let memory = crate::__private::decoding_add(memory, <$codec as crate::storage_codec::Value>::memory(child, $ftag)?)?;)*
                            Ok(memory)
                        })+
                        _ => unreachable!("variant tags are selected"),
                    }
                }
                #[allow(unused_variables)]
                fn decode_input(input: crate::__private::DecodeInput<'_>) -> std::result::Result<Self, crate::MessageError> {
                    <Self as crate::__private::Protobuf>::decoding_memory_input(input)?;
                    let (tag, child) = input
                        .variant(&[$($tag),+])?
                        .ok_or_else(|| crate::MessageError::borrowed(concat!("missing ", stringify!($ty), " variant")))?;
                    match tag {
                        $($tag => {
                            $(let $field = <$codec as crate::storage_codec::Value>::read(child, $ftag)?;)*
                            Ok($build)
                        })+
                        _ => unreachable!("variant tags are selected"),
                    }
                }
            }
        }
    };
}
pub(crate) use variants;

macro_rules! coded {
    ($ty:ident { $($variant:ident),+ $(,)? }) => {
        impl crate::storage_codec::Coded for $ty {
            const VARIANTS: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}
pub(crate) use coded;

/* === Type Aliases === */
type Result<T, E = MessageError> = std::result::Result<T, E>;

/* === Structs === */
pub(crate) struct Repeated<T>(PhantomData<fn() -> T>);
pub(crate) struct Nested<T>(PhantomData<fn() -> T>);
pub(crate) struct Optional<T>(PhantomData<fn() -> T>);
pub(crate) struct Id;
pub(crate) struct Bytes;
pub(crate) struct Unsigned;
pub(crate) struct Version;
pub(crate) struct Flag;
pub(crate) struct Signed;
pub(crate) struct Float;
pub(crate) struct Text;
pub(crate) struct Code<E>(PhantomData<fn() -> E>);
pub(crate) struct Index;
pub(crate) struct Count;
pub(crate) struct Wide;
pub(crate) struct WideNonZero;
pub(crate) struct WideSigned;
pub(crate) struct Wrap<A>(PhantomData<fn() -> A>);
pub(crate) struct Many<A>(PhantomData<fn() -> A>);
pub(crate) struct Both<A, B>(PhantomData<fn() -> (A, B)>);

/* === Enums === */

/* === Traits === */
#[cfg(feature = "grpc")]
pub(crate) trait Native:
    PostcardValue + crate::__private::Protobuf + MemorySize + Described
{
}
#[cfg(feature = "grpc")]
impl<T: PostcardValue + crate::__private::Protobuf + MemorySize + Described> Native for T {}
#[cfg(not(feature = "grpc"))]
pub(crate) trait Native: PostcardValue + MemorySize + Described {}
#[cfg(not(feature = "grpc"))]
impl<T: PostcardValue + MemorySize + Described> Native for T {}

pub(crate) trait Coded: Copy + PartialEq + Send + Sync + 'static {
    const VARIANTS: &'static [Self];
    fn code(&self) -> u32 {
        Self::VARIANTS
            .iter()
            .position(|variant| variant == self)
            .and_then(|index| u32::try_from(index).ok())
            .expect("a coded variant is listed")
    }
    fn from_code(code: u64) -> Result<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|index| Self::VARIANTS.get(index).copied())
            .ok_or_else(|| MessageError::borrowed("unknown storage enum code"))
    }
}

pub(crate) trait Value {
    type Native;
    const KIND: Kind;
    const CARDINALITY: Cardinality = Cardinality::Required;
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>);
    fn postcard_size(value: &Self::Native) -> usize;
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize>;
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native>;
    #[cfg(feature = "grpc")]
    fn write(value: &Self::Native, tag: u32, bytes: &mut Vec<u8>);
    #[cfg(feature = "grpc")]
    fn size(value: &Self::Native, tag: u32) -> usize;
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize>;
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self::Native>;
}

/* === Implementations === */
impl<T: Native> Value for Repeated<T> {
    postcard_field!();
    type Native = Vec<T>;
    const KIND: Kind = Nested::<T>::KIND;
    const CARDINALITY: Cardinality = Cardinality::Repeated;
    #[cfg(feature = "grpc")]
    fn write(values: &Vec<T>, tag: u32, bytes: &mut Vec<u8>) {
        for value in values {
            Nested::<T>::write(value, tag, bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(values: &Vec<T>, tag: u32) -> usize {
        values.iter().fold(0usize, |size, value| {
            size.saturating_add(Nested::<T>::size(value, tag))
        })
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let mut memory = 0usize;
        input.fields(&mut |field| {
            if field.tag == tag {
                let child = input.child(field.bytes()?)?;
                memory = crate::__private::decoding_add(memory, T::decoding_memory_input(child)?)?;
            }
            Ok(())
        })?;
        Ok(memory)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<T>> {
        let mut count = 0usize;
        input.fields(&mut |field| {
            if field.tag == tag {
                count = crate::__private::decoding_add(count, 1)?;
            }
            Ok(())
        })?;
        let mut values = crate::__private::decoding_vector(count)?;
        input.fields(&mut |field| {
            if field.tag == tag {
                values.push(T::decode_input(input.child(field.bytes()?)?)?);
            }
            Ok(())
        })?;
        Ok(values)
    }
}

impl<T: Native> Value for Nested<T> {
    postcard_field!();
    type Native = T;
    const KIND: Kind = Kind::Message {
        fields: Cow::Borrowed(T::FIELDS),
    };
    #[cfg(feature = "grpc")]
    fn write(value: &T, tag: u32, bytes: &mut Vec<u8>) {
        write_header(tag, Message::encoded_len(value), bytes);
        Message::encode_into(value, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &T, tag: u32) -> usize {
        length_delimited_size(tag, Message::encoded_len(value))
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        bytes_field(input, tag)?
            .ok_or_else(|| MessageError::borrowed("missing storage message field"))?;
        Ok(T::decoding_memory_input(input.field(tag)?)?.saturating_sub(std::mem::size_of::<T>()))
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<T> {
        T::decode_input(input.field(tag)?)
    }
}

impl<T: Value> Value for Optional<T> {
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        PostcardValue::write(&value.is_some(), bytes);
        if let Some(value) = value {
            T::postcard_write(value, bytes);
        }
    }
    fn postcard_size(value: &Self::Native) -> usize {
        1usize.saturating_add(value.as_ref().map_or(0, T::postcard_size))
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        input.nested(|input| {
            if input.value::<bool>()? {
                T::postcard_memory(input)
            } else {
                Ok(0)
            }
        })
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native> {
        input.nested(|input| {
            if input.value::<bool>()? {
                T::postcard_read(input).map(Some)
            } else {
                Ok(None)
            }
        })
    }

    type Native = Option<T::Native>;
    const KIND: Kind = T::KIND;
    const CARDINALITY: Cardinality = Cardinality::Optional;
    #[cfg(feature = "grpc")]
    fn write(value: &Self::Native, tag: u32, bytes: &mut Vec<u8>) {
        if let Some(value) = value {
            T::write(value, tag, bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(value: &Self::Native, tag: u32) -> usize {
        value.as_ref().map_or(0, |value| T::size(value, tag))
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        if present(input, tag)? {
            T::memory(input, tag)
        } else {
            Ok(0)
        }
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self::Native> {
        if present(input, tag)? {
            T::read(input, tag).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl Value for Id {
    postcard_field!();
    type Native = Identifier;
    const KIND: Kind = Kind::String;
    #[cfg(feature = "grpc")]
    fn write(value: &Identifier, tag: u32, bytes: &mut Vec<u8>) {
        write_bytes(tag, value.as_str().as_bytes(), bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &Identifier, tag: u32) -> usize {
        length_delimited_size(tag, value.as_str().len())
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Ok(text_field(input, tag)?.unwrap_or("").len())
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Identifier> {
        identifier(text_field(input, tag)?.unwrap_or(""))
    }
}

impl Value for Bytes {
    postcard_field!();
    type Native = Vec<u8>;
    const KIND: Kind = Kind::Bytes;
    #[cfg(feature = "grpc")]
    fn write(value: &Vec<u8>, tag: u32, bytes: &mut Vec<u8>) {
        write_bytes(tag, value, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &Vec<u8>, tag: u32) -> usize {
        length_delimited_size(tag, value.len())
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Ok(bytes_field(input, tag)?.unwrap_or(&[]).len())
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<u8>> {
        Ok(Box::<[u8]>::from(bytes_field(input, tag)?.unwrap_or(&[])).into_vec())
    }
}

impl Value for Unsigned {
    postcard_field!();
    type Native = u64;
    const KIND: Kind = Kind::U64;
    #[cfg(feature = "grpc")]
    fn write(value: &u64, tag: u32, bytes: &mut Vec<u8>) {
        crate::__private::prost::encoding::uint64::encode(tag, value, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &u64, tag: u32) -> usize {
        crate::__private::prost::encoding::uint64::encoded_len(tag, value)
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        uint_field(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<u64> {
        Ok(uint_field(input, tag)?.unwrap_or(0))
    }
}

impl Value for Version {
    postcard_field!();
    type Native = NonZeroU32;
    const KIND: Kind = Kind::U32;
    #[cfg(feature = "grpc")]
    fn write(value: &NonZeroU32, tag: u32, bytes: &mut Vec<u8>) {
        crate::__private::prost::encoding::uint32::encode(tag, &value.get(), bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &NonZeroU32, tag: u32) -> usize {
        crate::__private::prost::encoding::uint32::encoded_len(tag, &value.get())
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Self::read(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<NonZeroU32> {
        uint_field(input, tag)?
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or_else(|| MessageError::borrowed("invalid storage schema version"))
    }
}

macro_rules! varint {
    ($adapter:ty, $native:ty, $kind:ident, $encoding:ident, $from:expr) => {
        impl Value for $adapter {
            postcard_field!();
            type Native = $native;
            const KIND: Kind = Kind::$kind;
            #[cfg(feature = "grpc")]
            fn write(value: &$native, tag: u32, bytes: &mut Vec<u8>) {
                if *value != <$native>::default() {
                    crate::__private::prost::encoding::$encoding::encode(tag, value, bytes);
                }
            }
            #[cfg(feature = "grpc")]
            fn size(value: &$native, tag: u32) -> usize {
                if *value == <$native>::default() {
                    0
                } else {
                    crate::__private::prost::encoding::$encoding::encoded_len(tag, value)
                }
            }
            #[cfg(feature = "grpc")]
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
                Self::read(input, tag)?;
                Ok(0)
            }
            #[cfg(feature = "grpc")]
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<$native> {
                uint_field(input, tag)?.map_or(Ok(<$native>::default()), $from)
            }
        }
    };
}

varint!(Flag, bool, Bool, bool, |value: u64| match value {
    0 => Ok(false),
    1 => Ok(true),
    _ => Err(MessageError::borrowed("invalid storage boolean")),
});
varint!(Signed, i64, I64, int64, |value: u64| Ok(value as i64));

impl Value for Float {
    postcard_field!();
    type Native = f64;
    const KIND: Kind = Kind::F64;
    #[cfg(feature = "grpc")]
    fn write(value: &f64, tag: u32, bytes: &mut Vec<u8>) {
        if value.to_bits() != 0 {
            crate::__private::prost::encoding::double::encode(tag, value, bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(value: &f64, tag: u32) -> usize {
        if value.to_bits() == 0 {
            0
        } else {
            crate::__private::prost::encoding::double::encoded_len(tag, value)
        }
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Self::read(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<f64> {
        let mut value = 0.0;
        input.fields(&mut |field| {
            if field.tag == tag {
                let crate::__private::DecodeValue::Bits64(bits) =
                    field.scalar(crate::__private::DecodeScalar::Bits64)?
                else {
                    unreachable!("validated fixed-width scalar");
                };
                value = f64::from_bits(bits);
            }
            Ok(())
        })?;
        Ok(value)
    }
}

impl Value for Text {
    postcard_field!();
    type Native = String;
    const KIND: Kind = Kind::String;
    #[cfg(feature = "grpc")]
    fn write(value: &String, tag: u32, bytes: &mut Vec<u8>) {
        if !value.is_empty() {
            write_bytes(tag, value.as_bytes(), bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(value: &String, tag: u32) -> usize {
        if value.is_empty() {
            0
        } else {
            length_delimited_size(tag, value.len())
        }
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Ok(text_field(input, tag)?.unwrap_or("").len())
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<String> {
        Ok(Box::<str>::from(text_field(input, tag)?.unwrap_or("")).into_string())
    }
}

impl<E: Coded> Value for Code<E> {
    type Native = E;
    const KIND: Kind = Kind::U32;
    fn postcard_write(value: &E, bytes: &mut Vec<u8>) {
        PostcardValue::write(&value.code(), bytes);
    }
    fn postcard_size(value: &E) -> usize {
        PostcardValue::size(&value.code())
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        E::from_code(u64::from(input.value::<u32>()?))?;
        Ok(0)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<E> {
        E::from_code(u64::from(input.value::<u32>()?))
    }
    #[cfg(feature = "grpc")]
    fn write(value: &E, tag: u32, bytes: &mut Vec<u8>) {
        let code = value.code();
        if code != 0 {
            crate::__private::prost::encoding::uint32::encode(tag, &code, bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(value: &E, tag: u32) -> usize {
        let code = value.code();
        if code == 0 {
            0
        } else {
            crate::__private::prost::encoding::uint32::encoded_len(tag, &code)
        }
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Self::read(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<E> {
        E::from_code(uint_field(input, tag)?.unwrap_or(0))
    }
}

// Host Indices And Counts Travel As uint64; Decoding Never Truncates.
fn host_width(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| MessageError::borrowed("index exceeds host width"))
}

impl Value for Index {
    type Native = usize;
    const KIND: Kind = Kind::U64;
    fn postcard_write(value: &usize, bytes: &mut Vec<u8>) {
        PostcardValue::write(&(*value as u64), bytes);
    }
    fn postcard_size(value: &usize) -> usize {
        PostcardValue::size(&(*value as u64))
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        Self::postcard_read(input)?;
        Ok(0)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<usize> {
        host_width(input.value::<u64>()?)
    }
    #[cfg(feature = "grpc")]
    fn write(value: &usize, tag: u32, bytes: &mut Vec<u8>) {
        if *value != 0 {
            crate::__private::prost::encoding::uint64::encode(tag, &(*value as u64), bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(value: &usize, tag: u32) -> usize {
        if *value == 0 {
            0
        } else {
            crate::__private::prost::encoding::uint64::encoded_len(tag, &(*value as u64))
        }
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Self::read(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        host_width(uint_field(input, tag)?.unwrap_or(0))
    }
}

impl Value for Count {
    type Native = NonZeroUsize;
    const KIND: Kind = Kind::U64;
    fn postcard_write(value: &NonZeroUsize, bytes: &mut Vec<u8>) {
        Index::postcard_write(&value.get(), bytes);
    }
    fn postcard_size(value: &NonZeroUsize) -> usize {
        Index::postcard_size(&value.get())
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        Self::postcard_read(input)?;
        Ok(0)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<NonZeroUsize> {
        NonZeroUsize::new(Index::postcard_read(input)?)
            .ok_or_else(|| MessageError::borrowed("count must be nonzero"))
    }
    #[cfg(feature = "grpc")]
    fn write(value: &NonZeroUsize, tag: u32, bytes: &mut Vec<u8>) {
        Index::write(&value.get(), tag, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &NonZeroUsize, tag: u32) -> usize {
        Index::size(&value.get(), tag)
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Self::read(input, tag)?;
        Ok(0)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<NonZeroUsize> {
        NonZeroUsize::new(Index::read(input, tag)?)
            .ok_or_else(|| MessageError::borrowed("count must be nonzero"))
    }
}

// 128-Bit Values Travel As A Message Of Lossless High And Low Halves.
macro_rules! wide {
    ($adapter:ty, $native:ty, $high:ty, $high_codec:ty, $split:expr, $join:expr) => {
        impl Value for $adapter {
            type Native = $native;
            const KIND: Kind = Kind::Message {
                fields: Cow::Borrowed(&[
                    crate::schema::Field {
                        name: Cow::Borrowed("high"),
                        tag: 1,
                        kind: <$high_codec as Value>::KIND,
                        cardinality: Cardinality::Required,
                    },
                    crate::schema::Field {
                        name: Cow::Borrowed("low"),
                        tag: 2,
                        kind: Kind::U64,
                        cardinality: Cardinality::Required,
                    },
                ]),
            };
            fn postcard_write(value: &$native, bytes: &mut Vec<u8>) {
                let (high, low): ($high, u64) = ($split)(*value);
                PostcardValue::write(&high, bytes);
                PostcardValue::write(&low, bytes);
            }
            fn postcard_size(value: &$native) -> usize {
                let (high, low): ($high, u64) = ($split)(*value);
                PostcardValue::size(&high).saturating_add(PostcardValue::size(&low))
            }
            fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
                Self::postcard_read(input)?;
                Ok(0)
            }
            fn postcard_read(input: &mut PostcardReader<'_>) -> Result<$native> {
                let high = input.value::<$high>()?;
                let low = input.value::<u64>()?;
                ($join)(high, low)
            }
            #[cfg(feature = "grpc")]
            fn write(value: &$native, tag: u32, bytes: &mut Vec<u8>) {
                let (high, low): ($high, u64) = ($split)(*value);
                let length =
                    <$high_codec as Value>::size(&high, 1).saturating_add(Unsigned::size(&low, 2));
                write_header(tag, length, bytes);
                <$high_codec as Value>::write(&high, 1, bytes);
                Unsigned::write(&low, 2, bytes);
            }
            #[cfg(feature = "grpc")]
            fn size(value: &$native, tag: u32) -> usize {
                let (high, low): ($high, u64) = ($split)(*value);
                length_delimited_size(
                    tag,
                    <$high_codec as Value>::size(&high, 1).saturating_add(Unsigned::size(&low, 2)),
                )
            }
            #[cfg(feature = "grpc")]
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
                Self::read(input, tag)?;
                Ok(0)
            }
            #[cfg(feature = "grpc")]
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<$native> {
                let child = input.child(bytes_field(input, tag)?.unwrap_or(&[]))?;
                let high = <$high_codec as Value>::read(child, 1)?;
                let low = uint_field(child, 2)?.unwrap_or(0);
                ($join)(high, low)
            }
        }
    };
}

wide!(
    Wide,
    u128,
    u64,
    Unsigned,
    |value: u128| ((value >> 64) as u64, value as u64),
    |high: u64, low: u64| Ok::<_, MessageError>((u128::from(high) << 64) | u128::from(low))
);
wide!(
    WideNonZero,
    std::num::NonZeroU128,
    u64,
    Unsigned,
    |value: std::num::NonZeroU128| ((value.get() >> 64) as u64, value.get() as u64),
    |high: u64, low: u64| std::num::NonZeroU128::new((u128::from(high) << 64) | u128::from(low))
        .ok_or_else(|| MessageError::borrowed("wide value must be nonzero"))
);
wide!(
    WideSigned,
    i128,
    i64,
    Signed,
    |value: i128| ((value >> 64) as i64, value as u64),
    |high: i64, low: u64| Ok::<_, MessageError>((i128::from(high) << 64) | i128::from(low))
);

// One Message Holding A Single Field At Tag 1; Absence Reads As Empty.
impl<A: Value> Value for Wrap<A> {
    type Native = A::Native;
    const KIND: Kind = Kind::Message {
        fields: Cow::Borrowed(&[crate::schema::Field {
            name: Cow::Borrowed("value"),
            tag: 1,
            kind: A::KIND,
            cardinality: A::CARDINALITY,
        }]),
    };
    fn postcard_write(value: &A::Native, bytes: &mut Vec<u8>) {
        A::postcard_write(value, bytes);
    }
    fn postcard_size(value: &A::Native) -> usize {
        A::postcard_size(value)
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        A::postcard_memory(input)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<A::Native> {
        A::postcard_read(input)
    }
    #[cfg(feature = "grpc")]
    fn write(value: &A::Native, tag: u32, bytes: &mut Vec<u8>) {
        write_header(tag, A::size(value, 1), bytes);
        A::write(value, 1, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &A::Native, tag: u32) -> usize {
        length_delimited_size(tag, A::size(value, 1))
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        A::memory(input.child(bytes_field(input, tag)?.unwrap_or(&[]))?, 1)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<A::Native> {
        A::read(input.child(bytes_field(input, tag)?.unwrap_or(&[]))?, 1)
    }
}

// Repeated Messages, Each Holding One Element At Tag 1.
impl<A: Value> Value for Many<A> {
    type Native = Vec<A::Native>;
    const KIND: Kind = Wrap::<A>::KIND;
    const CARDINALITY: Cardinality = Cardinality::Repeated;
    fn postcard_write(values: &Vec<A::Native>, bytes: &mut Vec<u8>) {
        PostcardValue::write(&(values.len() as u64), bytes);
        for value in values {
            A::postcard_write(value, bytes);
        }
    }
    fn postcard_size(values: &Vec<A::Native>) -> usize {
        values.iter().fold(
            PostcardValue::size(&(values.len() as u64)),
            |size, value| size.saturating_add(A::postcard_size(value)),
        )
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        let count = host_width(input.value::<u64>()?)?;
        input.claim(count)?;
        let mut memory =
            crate::__private::decoding_multiply(count, std::mem::size_of::<A::Native>())?;
        for _ in 0..count {
            memory = crate::__private::decoding_add(memory, A::postcard_memory(input)?)?;
        }
        Ok(memory)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Vec<A::Native>> {
        let count = host_width(input.value::<u64>()?)?;
        input.claim(count)?;
        let mut values = crate::__private::decoding_vector(count)?;
        for _ in 0..count {
            values.push(A::postcard_read(input)?);
        }
        Ok(values)
    }
    #[cfg(feature = "grpc")]
    fn write(values: &Vec<A::Native>, tag: u32, bytes: &mut Vec<u8>) {
        for value in values {
            Wrap::<A>::write(value, tag, bytes);
        }
    }
    #[cfg(feature = "grpc")]
    fn size(values: &Vec<A::Native>, tag: u32) -> usize {
        values.iter().fold(0usize, |size, value| {
            size.saturating_add(Wrap::<A>::size(value, tag))
        })
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let mut memory = 0usize;
        input.fields(&mut |field| {
            if field.tag == tag {
                let child = input.child(field.bytes()?)?;
                memory = crate::__private::decoding_add(
                    memory,
                    std::mem::size_of::<A::Native>().saturating_add(A::memory(child, 1)?),
                )?;
            }
            Ok(())
        })?;
        Ok(memory)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<A::Native>> {
        let mut count = 0usize;
        input.fields(&mut |field| {
            if field.tag == tag {
                count = crate::__private::decoding_add(count, 1)?;
            }
            Ok(())
        })?;
        let mut values = crate::__private::decoding_vector(count)?;
        input.fields(&mut |field| {
            if field.tag == tag {
                values.push(A::read(input.child(field.bytes()?)?, 1)?);
            }
            Ok(())
        })?;
        Ok(values)
    }
}

// One Message Holding A Pair At Tags 1 And 2.
impl<A: Value, B: Value> Value for Both<A, B> {
    type Native = (A::Native, B::Native);
    const KIND: Kind = Kind::Message {
        fields: Cow::Borrowed(&[
            crate::schema::Field {
                name: Cow::Borrowed("first"),
                tag: 1,
                kind: A::KIND,
                cardinality: A::CARDINALITY,
            },
            crate::schema::Field {
                name: Cow::Borrowed("second"),
                tag: 2,
                kind: B::KIND,
                cardinality: B::CARDINALITY,
            },
        ]),
    };
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        A::postcard_write(&value.0, bytes);
        B::postcard_write(&value.1, bytes);
    }
    fn postcard_size(value: &Self::Native) -> usize {
        A::postcard_size(&value.0).saturating_add(B::postcard_size(&value.1))
    }
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        crate::__private::decoding_add(A::postcard_memory(input)?, B::postcard_memory(input)?)
    }
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native> {
        Ok((A::postcard_read(input)?, B::postcard_read(input)?))
    }
    #[cfg(feature = "grpc")]
    fn write(value: &Self::Native, tag: u32, bytes: &mut Vec<u8>) {
        let length = A::size(&value.0, 1).saturating_add(B::size(&value.1, 2));
        write_header(tag, length, bytes);
        A::write(&value.0, 1, bytes);
        B::write(&value.1, 2, bytes);
    }
    #[cfg(feature = "grpc")]
    fn size(value: &Self::Native, tag: u32) -> usize {
        length_delimited_size(
            tag,
            A::size(&value.0, 1).saturating_add(B::size(&value.1, 2)),
        )
    }
    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let child = input.child(bytes_field(input, tag)?.unwrap_or(&[]))?;
        crate::__private::decoding_add(A::memory(child, 1)?, B::memory(child, 2)?)
    }
    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self::Native> {
        let child = input.child(bytes_field(input, tag)?.unwrap_or(&[]))?;
        Ok((A::read(child, 1)?, B::read(child, 2)?))
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
pub(crate) fn length_delimited_size(tag: u32, length: usize) -> usize {
    field_len(length)
        .saturating_add(crate::__private::prost::encoding::key_len(tag).saturating_sub(1))
}

#[cfg(feature = "grpc")]
fn present(input: DecodeInput<'_>, tag: u32) -> Result<bool> {
    let mut present = false;
    input.fields(&mut |field| {
        present |= field.tag == tag;
        Ok(())
    })?;
    Ok(present)
}

/* === Tests === */
