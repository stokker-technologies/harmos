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
use std::borrow::Cow;
// Third Party Imports

// Local Imports
use crate::MemorySize;
#[cfg(feature = "grpc")]
use crate::message::{
    DecodeField, DecodeInput, DecodeScalar, DecodeValue, decoding_add, decoding_vector,
};
use crate::schema::{Cardinality, Described, Field, Kind};
#[cfg(feature = "grpc")]
use crate::{MessageError, Protobuf as Message};

/* === Constants === */

/* === Macros === */
macro_rules! payload {
    ($name:ident, $( $field:ident: $ty:ty => ($tag:literal, $kind:expr, $cardinality:ident)),* $(,)?) => {
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct $name { $(pub $field: $ty),* }
        impl Described for $name {
            const FIELDS: &'static [Field] = &[$(Field {
                name: Cow::Borrowed(stringify!($field)), tag: $tag,
                kind: $kind, cardinality: Cardinality::$cardinality,
            }),*];
        }
        crate::message::postcard_product!($name { $($field: $ty),* });
        crate::with_protobuf! {
        impl Message for $name {
            fn encode_to_vec(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(self.encoded_len());
                self.encode_into(&mut bytes);
                bytes
            }
            fn encode_into(&self, bytes: &mut Vec<u8>) {
                $(self.$field.write($tag, bytes);)*
            }
            fn encoded_len(&self) -> usize {
                0usize$(.saturating_add(self.$field.size($tag)))*
            }
            fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
                Self::decoding_memory_input(DecodeInput::new(bytes))
            }
            fn decoding_memory_input(input: DecodeInput<'_>) -> Result<usize, MessageError> {
                let owned = std::mem::size_of::<Self>();
                $(let owned = decoding_add(owned, <$ty as FieldCodec>::memory(input, $tag)?)?;)*
                Ok(owned)
            }
            fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
                Self::decode_input(DecodeInput::new(bytes))
            }
            fn decode_input(input: DecodeInput<'_>) -> Result<Self, MessageError> {
                // Validate Every Field Before Allocating The Native Value
                Self::decoding_memory_input(input)?;
                Ok(Self { $($field: <$ty as FieldCodec>::read(input, $tag)?),* })
            }
        }
        }
        impl MemorySize for $name {
            fn memory_size(&self) -> usize {
                std::mem::size_of::<Self>()$(.saturating_add(
                    self.$field.memory_size().saturating_sub(std::mem::size_of::<$ty>())
                ))*
            }
        }
    };
}

// Keep Historical Host Tags Independent Of Authored Field-Name Tags
crate::with_protobuf! {
macro_rules! scalar {
    ($ty:ty, $encoding:ident, $inspect:expr, $read:expr) => {
        impl FieldCodec for $ty {
            fn write(&self, tag: u32, bytes: &mut Vec<u8>) {
                if self != &Self::default() {
                    prost::encoding::$encoding::encode(tag, self, bytes);
                }
            }
            fn size(&self, tag: u32) -> usize {
                if self == &Self::default() {
                    0
                } else {
                    prost::encoding::$encoding::encoded_len(tag, self)
                }
            }
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError> {
                let mut owned = 0;
                selected(input, tag, &mut |field| {
                    owned = decoding_add(owned, ($inspect)(field)?)?;
                    Ok(())
                })?;
                Ok(owned)
            }
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self, MessageError> {
                let mut value = Self::default();
                selected(input, tag, &mut |field| {
                    value = ($read)(field)?;
                    Ok(())
                })?;
                Ok(value)
            }
        }
        impl FieldCodec for Option<$ty> {
            fn write(&self, tag: u32, bytes: &mut Vec<u8>) {
                if let Some(value) = self {
                    prost::encoding::$encoding::encode(tag, value, bytes);
                }
            }
            fn size(&self, tag: u32) -> usize {
                self.as_ref().map_or(0, |value| {
                    prost::encoding::$encoding::encoded_len(tag, value)
                })
            }
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError> {
                <$ty as FieldCodec>::memory(input, tag)
            }
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self, MessageError> {
                let mut value = None;
                selected(input, tag, &mut |field| {
                    value = Some(($read)(field)?);
                    Ok(())
                })?;
                Ok(value)
            }
        }
    };
}

scalar!(
    String,
    string,
    |field: DecodeField<'_>| field.text().map(str::len),
    |field: DecodeField<'_>| field
        .text()
        .map(|text| Box::<str>::from(text).into_string())
);

scalar!(
    Vec<u8>,
    bytes,
    |field: DecodeField<'_>| field.bytes().map(<[u8]>::len),
    |field: DecodeField<'_>| field
        .bytes()
        .map(|bytes| Box::<[u8]>::from(bytes).into_vec())
);

scalar!(
    u32,
    uint32,
    |field: DecodeField<'_>| field.scalar(DecodeScalar::Varint).map(|_| 0),
    |field: DecodeField<'_>| unsigned(field).map(|value| value as u32)
);

scalar!(
    u64,
    uint64,
    |field: DecodeField<'_>| field.scalar(DecodeScalar::Varint).map(|_| 0),
    |field: DecodeField<'_>| unsigned(field)
);

scalar!(
    i64,
    int64,
    |field: DecodeField<'_>| field.scalar(DecodeScalar::Varint).map(|_| 0),
    |field: DecodeField<'_>| unsigned(field).map(|value| value as i64)
);

scalar!(
    f64,
    double,
    |field: DecodeField<'_>| field.scalar(DecodeScalar::Bits64).map(|_| 0),
    |field: DecodeField<'_>| match field.scalar(DecodeScalar::Bits64)? {
        DecodeValue::Bits64(value) => Ok(f64::from_bits(value)),
        _ => unreachable!("validated fixed-width scalar"),
    }
);

macro_rules! repeated {
    ($ty:ty) => {
        impl FieldCodec for Vec<$ty> {
            fn write(&self, tag: u32, bytes: &mut Vec<u8>) {
                for value in self {
                    prost::encoding::encode_key(
                        tag,
                        prost::encoding::WireType::LengthDelimited,
                        bytes,
                    );
                    prost::encoding::encode_varint(value.encoded_len() as u64, bytes);
                    value.encode_into(bytes);
                }
            }
            fn size(&self, tag: u32) -> usize {
                self.iter().fold(0usize, |total, value| {
                    let length = value.encoded_len();
                    total
                        .saturating_add(prost::encoding::key_len(tag))
                        .saturating_add(prost::encoding::encoded_len_varint(length as u64))
                        .saturating_add(length)
                })
            }
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError> {
                let mut count = 0;
                let mut owned = 0;
                selected(input, tag, &mut |field| {
                    count = decoding_add(count, 1)?;
                    let child = input.child(field.bytes()?)?;
                    owned = decoding_add(owned, <$ty>::decoding_memory_input(child)?)?;
                    Ok(())
                })?;
                let layout = std::alloc::Layout::array::<$ty>(count).map_err(|_| {
                    MessageError::borrowed("decoding container exceeds address space")
                })?;
                decoding_add(owned, layout.size())
            }
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self, MessageError> {
                let mut count = 0;
                selected(input, tag, &mut |field| {
                    field.bytes()?;
                    count = decoding_add(count, 1)?;
                    Ok(())
                })?;
                let mut values = decoding_vector(count)?;
                selected(input, tag, &mut |field| {
                    values.push(<$ty>::decode_input(input.child(field.bytes()?)?)?);
                    Ok(())
                })?;
                Ok(values)
            }
        }
    };
}

repeated!(Header);

repeated!(Remembered);

repeated!(Measured);

}

payload! { Publication,
    stream: String => (1, Kind::String, Required),
    schema: u32 => (2, Kind::U32, Required),
    sequence: Option<u64> => (3, Kind::U64, Optional),
    published_unix_micros: Option<i64> => (4, Kind::I64, Optional),
    context: Option<String> => (5, Kind::String, Optional),
    payload: Vec<u8> => (6, Kind::Bytes, Required),
}

payload! { RecordRequest,
    name: String => (1, Kind::String, Required),
    version: u32 => (2, Kind::U32, Required),
    payload: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { Receipt,
    position: u64 => (1, Kind::U64, Required),
    correlation: u64 => (2, Kind::U64, Required),
}

payload! { RecallRequest,
    name: String => (1, Kind::String, Required),
    after: u64 => (2, Kind::U64, Required),
    limit: u32 => (3, Kind::U32, Required),
}

payload! { Remembered,
    position: u64 => (1, Kind::U64, Required),
    name: String => (2, Kind::String, Required),
    version: u32 => (3, Kind::U32, Required),
    payload: Vec<u8> => (4, Kind::Bytes, Required),
}

payload! { RecallResponse,
    rows: Vec<Remembered> => (1, Kind::Message { fields: Cow::Borrowed(Remembered::FIELDS) }, Repeated),
}

payload! { InvokeRequest,
    grant: String => (1, Kind::String, Required),
    key: String => (2, Kind::String, Required),
    payload: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { Header,
    name: String => (1, Kind::String, Required),
    value: String => (2, Kind::String, Required),
}

payload! { EgressRequest,
    name: String => (1, Kind::String, Required),
    method: String => (2, Kind::String, Required),
    path: String => (3, Kind::String, Required),
    headers: Vec<Header> => (4, Kind::Message { fields: Cow::Borrowed(Header::FIELDS) }, Repeated),
    body: Vec<u8> => (5, Kind::Bytes, Required),
}

payload! { Fetched,
    status: u32 => (1, Kind::U32, Required),
    headers: Vec<Header> => (2, Kind::Message { fields: Cow::Borrowed(Header::FIELDS) }, Repeated),
    body: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { SurveyRequest,
    stream: String => (1, Kind::String, Required),
    first: u64 => (2, Kind::U64, Required),
    last: u64 => (3, Kind::U64, Required),
    aggregate: String => (4, Kind::String, Required),
}

payload! { Measured,
    field: String => (1, Kind::String, Required),
    value: f64 => (2, Kind::F64, Required),
}

payload! { SurveyResponse,
    rows: Vec<Measured> => (1, Kind::Message { fields: Cow::Borrowed(Measured::FIELDS) }, Repeated),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
// Field Memory Excludes Inline Storage Already Charged By The Payload
#[cfg(feature = "grpc")]
trait FieldCodec: Sized {
    fn write(&self, tag: u32, bytes: &mut Vec<u8>);
    fn size(&self, tag: u32) -> usize;
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError>;
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Self, MessageError>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn selected<'a>(
    input: DecodeInput<'a>,
    tag: u32,
    visit: &mut dyn FnMut(DecodeField<'a>) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    input.fields(&mut |field| {
        if field.tag == tag {
            visit(field)?;
        }
        Ok(())
    })
}

#[cfg(feature = "grpc")]
fn unsigned(field: DecodeField<'_>) -> Result<u64, MessageError> {
    match field.scalar(DecodeScalar::Varint)? {
        DecodeValue::Varint(value) => Ok(value),
        _ => unreachable!("validated unsigned scalar"),
    }
}

/* === Tests === */
