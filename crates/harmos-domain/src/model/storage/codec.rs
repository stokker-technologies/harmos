/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Codecs Retained for the Deferred Private Guest Hash Callee.

/* === Imports === */

use crate::__private::{PostcardReader, PostcardValue};
use crate::schema::declaration::{self, Kind};
use crate::schema::{Cardinality, Described};
use crate::{MemorySize, MessageError};
use std::marker::PhantomData;

/* === Macros === */

#[doc(hidden)]
#[macro_export]
macro_rules! storage_product {
    ($ty:ty { $($field:ident: $codec:ty => $tag:literal),* $(,)? }) => {
        $crate::model::storage::codec::product!($ty { $($field: $codec => $tag),* }, check = |_: &$ty| Ok(()));
    };
    ($ty:ty { $($field:ident: $codec:ty => $tag:literal),* $(,)? }, check = $check:expr) => {
        impl $crate::schema::Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [$crate::schema::declaration::Field] = &[$($crate::schema::declaration::Field {
                name: std::borrow::Cow::Borrowed(stringify!($field)),
                tag: $tag,
                kind: <$codec as $crate::model::storage::codec::Value>::KIND,
                cardinality: <$codec as $crate::model::storage::codec::Value>::CARDINALITY,
            }),*];

            const MARKER: &'static [u8] = &$crate::schema::declaration::encode::<{
                $crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
}
        #[allow(unused_variables)]
        impl $crate::__private::PostcardValue for $ty {
            fn write(&self, bytes: &mut Vec<u8>) {
                $(<$codec as $crate::model::storage::codec::Value>::postcard_write(&self.$field, bytes);)*
            }
            fn size(&self) -> usize {
                0usize$(.saturating_add(<$codec as $crate::model::storage::codec::Value>::postcard_size(&self.$field)))*
            }
            fn memory(input: &mut $crate::__private::PostcardReader<'_>) -> std::result::Result<usize, $crate::MessageError> {
                input.nested(|input| {
                    let memory = std::mem::size_of::<Self>();
                    $(let memory = $crate::__private::decoding_add(memory, <$codec as $crate::model::storage::codec::Value>::postcard_memory(input)?)?;)*
                    Ok(memory)
                })
            }
            fn read(input: &mut $crate::__private::PostcardReader<'_>) -> std::result::Result<Self, $crate::MessageError> {
                let value = input.nested(|input| Ok(Self { $($field: <$codec as $crate::model::storage::codec::Value>::postcard_read(input)?),* }))?;
                let check: fn(&Self) -> std::result::Result<(), $crate::MessageError> = $check;
                check(&value)?;
                Ok(value)
            }
        }
    };
}

pub use storage_product as product;

macro_rules! postcard_field {
    () => {
        fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
            crate::__private::PostcardValue::write(value, bytes);
        }
        fn postcard_size(value: &Self::Native) -> usize {
            crate::__private::PostcardValue::size(value)
        }
        fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, crate::MessageError> {
            Ok(<Self::Native as crate::__private::PostcardValue>::memory(input)?.saturating_sub(std::mem::size_of::<Self::Native>()))
        }
        fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<Self::Native, crate::MessageError> {
            <Self::Native as crate::__private::PostcardValue>::read(input)
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! storage_variants {
    ($ty:ty { $($index:literal, $tag:literal, $name:literal: $pat:pat => { $($field:ident: $codec:ty => $ftag:literal),* $(,)? } => $build:expr;)+ }) => {
        const _: () = {
        struct Variant<const TAG: u32>;
        $(impl $crate::schema::Described for Variant<$tag> {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty), "::", $name);
            const FIELDS: &'static [$crate::schema::declaration::Field] = &[$($crate::schema::declaration::Field {
                name: std::borrow::Cow::Borrowed(stringify!($field)), tag: $ftag,
                kind: <$codec as $crate::model::storage::codec::Value>::KIND,
                cardinality: <$codec as $crate::model::storage::codec::Value>::CARDINALITY,
            }),*];
            const MARKER: &'static [u8] = &$crate::schema::declaration::encode::<{
                $crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
        })+
        impl $crate::schema::Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [$crate::schema::declaration::Field] = &[$($crate::schema::declaration::Field {
                name: std::borrow::Cow::Borrowed($name),
                tag: $tag,
                cardinality: $crate::schema::Cardinality::Variant,
                kind: $crate::schema::declaration::Kind::Message {
                    definition: <Variant<$tag> as $crate::schema::Described>::DEFINITION,
                },
            }),+];

            // Every Storage Oneof Requires Its Variant; An Absent One Is Refused.
            const DEFAULT_VARIANT: Option<usize> = None;
            const MARKER: &'static [u8] = &$crate::schema::declaration::encode::<{
                $crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
}
        impl $crate::__private::PostcardValue for $ty {
            #[allow(unused_variables)]
            fn write(&self, bytes: &mut Vec<u8>) {
                match self {
                    $($pat => {
                        $crate::__private::PostcardValue::write(&($index as u32), bytes);
                        $(<$codec as $crate::model::storage::codec::Value>::postcard_write($field, bytes);)*
                    })+
                }
            }
            #[allow(unused_variables)]
            fn size(&self) -> usize {
                match self {
                    $($pat => $crate::__private::PostcardValue::size(&($index as u32))
                        $(.saturating_add(<$codec as $crate::model::storage::codec::Value>::postcard_size($field)))*,)+
                }
            }
            fn memory(input: &mut $crate::__private::PostcardReader<'_>) -> std::result::Result<usize, $crate::MessageError> {
                input.nested(|input| {
                    let memory = std::mem::size_of::<Self>();
                    match input.value::<u32>()? {
                        $($index => {
                            $(let memory = $crate::__private::decoding_add(memory, <$codec as $crate::model::storage::codec::Value>::postcard_memory(input)?)?;)*
                            Ok(memory)
                        })+
                        _ => Err($crate::MessageError::borrowed(concat!("unknown ", stringify!($ty), " variant"))),
                    }
                })
            }
            fn read(input: &mut $crate::__private::PostcardReader<'_>) -> std::result::Result<Self, $crate::MessageError> {
                input.nested(|input| match input.value::<u32>()? {
                    $($index => {
                        $(let $field = <$codec as $crate::model::storage::codec::Value>::postcard_read(input)?;)*
                        Ok($build)
                    })+
                    _ => Err($crate::MessageError::borrowed(concat!("unknown ", stringify!($ty), " variant"))),
                })
            }
        }
        };
    };
}
pub use storage_variants as variants;

/* === Type Aliases === */

type Result<T, E = MessageError> = std::result::Result<T, E>;

/* === Structs === */

pub struct Nested<T>(PhantomData<fn() -> T>);
pub struct Optional<T>(PhantomData<fn() -> T>);
pub struct Bytes;
pub struct Unsigned;

/* === Traits === */

pub trait Native: PostcardValue + MemorySize + Described {}
impl<T: PostcardValue + MemorySize + Described> Native for T {}

pub trait Value: 'static {
    const NAME: &'static str;
    type Native;
    const KIND: Kind;
    const CARDINALITY: Cardinality = Cardinality::Required;
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>);
    fn postcard_size(value: &Self::Native) -> usize;
    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize>;
    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native>;
}

/* === Implementations === */

impl<T: Native> Value for Nested<T> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Nested)), &[T::NAME]).as_str();
    postcard_field!();
    type Native = T;
    const KIND: Kind = Kind::Message { definition: T::DEFINITION };
}

impl<T: Value> Value for Optional<T> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Optional)), &[T::NAME]).as_str();
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
        input.nested(|input| if input.value::<bool>()? { T::postcard_memory(input) } else { Ok(0) })
    }

    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native> {
        input.nested(|input| if input.value::<bool>()? { T::postcard_read(input).map(Some) } else { Ok(None) })
    }

    type Native = Option<T::Native>;
    const KIND: Kind = T::KIND;
    const CARDINALITY: Cardinality = Cardinality::Optional;
}

impl Value for Bytes {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Bytes));
    postcard_field!();
    type Native = Vec<u8>;
    const KIND: Kind = Kind::Bytes;
}

impl Value for Unsigned {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Unsigned));
    postcard_field!();
    type Native = u64;
    const KIND: Kind = Kind::U64;
}
