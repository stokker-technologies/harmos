/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod timestamp;

/* === Re-Exports === */

pub use timestamp::Time;

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::__private::{PostcardReader, PostcardValue};
use crate::schema::declaration::{self, Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{Identifier, MemorySize, MessageError};

/* === Constants === */

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
pub(crate) use postcard_field;

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

#[doc(hidden)]
#[macro_export]
macro_rules! storage_coded {
    ($ty:ident { $($variant:ident),+ $(,)? }) => {
        impl $crate::model::storage::codec::Coded for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const VARIANTS: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}
pub use storage_coded as coded;

/* === Type Aliases === */

type Result<T, E = MessageError> = std::result::Result<T, E>;

/* === Structs === */

pub struct Repeated<T>(PhantomData<fn() -> T>);
pub struct Nested<T>(PhantomData<fn() -> T>);
pub struct Shared<T>(PhantomData<fn() -> T>);
pub struct Optional<T>(PhantomData<fn() -> T>);
pub struct Id;
pub struct Bytes;
pub struct Unsigned;
pub struct Percent;
pub struct Version;
pub struct Flag;
pub struct Signed;
pub struct Float;
pub struct Text;
pub struct Code<E>(PhantomData<fn() -> E>);
pub struct Index;
pub struct Wide;
pub struct WideNonZero;
pub struct WideSigned;
pub struct Wrap<A>(PhantomData<fn() -> A>);
pub struct Many<A>(PhantomData<fn() -> A>);

/* === Enums === */

/* === Traits === */

pub trait Native: PostcardValue + MemorySize + Described {}
impl<T: PostcardValue + MemorySize + Described> Native for T {}

pub trait Coded: Copy + PartialEq + Send + Sync + 'static {
    const NAME: &'static str;
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

impl<T: Native> Value for Repeated<T> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Repeated)), &[T::NAME]).as_str();
    postcard_field!();
    type Native = Vec<T>;
    const KIND: Kind = Nested::<T>::KIND;
    const CARDINALITY: Cardinality = Cardinality::Repeated;
}

impl<T: Native> Value for Nested<T> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Nested)), &[T::NAME]).as_str();
    postcard_field!();
    type Native = T;
    const KIND: Kind = Kind::Message { definition: T::DEFINITION };
}

impl<T: Native> Value for Shared<T> {
    const NAME: &'static str = Nested::<T>::NAME;
    type Native = Arc<T>;
    const KIND: Kind = Nested::<T>::KIND;

    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        T::write(value, bytes);
    }

    fn postcard_size(value: &Self::Native) -> usize {
        T::size(value)
    }

    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        crate::__private::decoding_add(T::memory(input)?, 2 * std::mem::size_of::<usize>())
    }

    fn postcard_read(input: &mut PostcardReader<'_>) -> Result<Self::Native> {
        T::read(input).map(Arc::new)
    }
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

impl Value for Id {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Id));
    postcard_field!();
    type Native = Identifier;
    const KIND: Kind = Kind::String;
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

// A whole percentage rank, carried as a protobuf `uint32`; the rank's range is
// a plan rule, so any byte decodes and the plan check refuses above 100.
impl Value for Percent {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Percent));
    postcard_field!();
    type Native = u8;
    const KIND: Kind = Kind::U32;
}

impl Value for Version {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Version));
    postcard_field!();
    type Native = NonZeroU32;
    const KIND: Kind = Kind::U32;
}

macro_rules! varint {
    ($adapter:ty, $native:ty, $kind:ident, $encoding:ident, $from:expr) => {
        impl Value for $adapter {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($adapter));
            postcard_field!();
            type Native = $native;
            const KIND: Kind = Kind::$kind;
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
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Float));
    postcard_field!();
    type Native = f64;
    const KIND: Kind = Kind::F64;
}

impl Value for Text {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Text));
    postcard_field!();
    type Native = String;
    const KIND: Kind = Kind::String;
}

impl<E: Coded> Value for Code<E> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Code)), &[E::NAME]).as_str();
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
}

// Host Indices And Counts Travel As uint64; Decoding Never Truncates.
fn host_width(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| MessageError::borrowed("index exceeds host width"))
}

impl Value for Index {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Index));
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
}

// 128-Bit Values Travel As A Message Of Lossless High And Low Halves.
macro_rules! wide {
    ($adapter:ty, $native:ty, $high:ty, $high_codec:ty, $split:expr, $join:expr) => {
        impl Described for $adapter {
            const NAME: &'static str = <Self as Value>::NAME;
            const FIELDS: &'static [Field] = &[
                crate::schema::declaration::Field {
                    name: Cow::Borrowed("high"),
                    tag: 1,
                    kind: <$high_codec as Value>::KIND,
                    cardinality: Cardinality::Required,
                },
                crate::schema::declaration::Field {
                    name: Cow::Borrowed("low"),
                    tag: 2,
                    kind: Kind::U64,
                    cardinality: Cardinality::Required,
                },
            ];
            const MARKER: &'static [u8] = &declaration::encode::<{ declaration::encoded_len(<Self as Described>::NAME, Self::FIELDS) }>(
                <Self as Described>::NAME,
                Self::FIELDS,
            );
        }
        impl Value for $adapter {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($adapter));
            type Native = $native;
            const KIND: Kind = Kind::Message { definition: <Self as Described>::DEFINITION };
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
        }
    };
}

wide!(Wide, u128, u64, Unsigned, |value: u128| ((value >> 64) as u64, value as u64), |high: u64, low: u64| Ok::<_, MessageError>(
    (u128::from(high) << 64) | u128::from(low)
));
wide!(
    WideNonZero,
    std::num::NonZeroU128,
    u64,
    Unsigned,
    |value: std::num::NonZeroU128| ((value.get() >> 64) as u64, value.get() as u64),
    |high: u64, low: u64| std::num::NonZeroU128::new((u128::from(high) << 64) | u128::from(low))
        .ok_or_else(|| MessageError::borrowed("wide value must be nonzero"))
);
wide!(WideSigned, i128, i64, Signed, |value: i128| ((value >> 64) as i64, value as u64), |high: i64, low: u64| Ok::<_, MessageError>(
    (i128::from(high) << 64) | i128::from(low)
));

// One Message Holding A Single Field At Tag 1; Absence Reads As Empty.
impl<A: Value> Described for Wrap<A> {
    const NAME: &'static str = <Self as Value>::NAME;
    const FIELDS: &'static [Field] =
        &[crate::schema::declaration::Field { name: Cow::Borrowed("value"), tag: 1, kind: A::KIND, cardinality: A::CARDINALITY }];
    const MARKER: &'static [u8] = declaration::encode_buffer::<2048>(<Self as Described>::NAME, Self::FIELDS)
        .split_at(declaration::encoded_len(<Self as Described>::NAME, Self::FIELDS))
        .0;
}
impl<A: Value> Value for Wrap<A> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Wrap)), &[A::NAME]).as_str();
    type Native = A::Native;
    const KIND: Kind = Kind::Message { definition: <Self as Described>::DEFINITION };
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
}

// Repeated Messages, Each Holding One Element At Tag 1.
impl<A: Value> Value for Many<A> {
    const NAME: &'static str = declaration::Name::new(concat!(module_path!(), "::", stringify!(Many)), &[A::NAME]).as_str();
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
        values.iter().fold(PostcardValue::size(&(values.len() as u64)), |size, value| size.saturating_add(A::postcard_size(value)))
    }

    fn postcard_memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        let count = host_width(input.value::<u64>()?)?;
        input.claim(count)?;
        let mut memory = crate::__private::decoding_multiply(count, std::mem::size_of::<A::Native>())?;
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
}

// One Message Holding A Pair At Tags 1 And 2.
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
