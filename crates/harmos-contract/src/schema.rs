/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod encoding;
mod payload;
#[cfg(feature = "grpc")]
mod protobuf;

#[cfg(feature = "reflection")]
pub mod reflection;

/* === Re-Exports === */
pub use payload::{Event, PathElement, PayloadError, Scalar, Validator, VisitError};
#[cfg(feature = "grpc")]
pub use protobuf::{PayloadError as ProtobufPayloadError, Validator as ProtobufValidator};

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */
pub const MAX_FIELDS: usize = 4096;

pub const MAX_DEPTH: usize = 32;

pub const MAX_NAME_BYTES: usize = 256;

/* === Macros === */
// Actual Prost Scalar Wrapper Payloads, With Their Fixed Field-One Identity
macro_rules! wrapper {
    ($($ty:ty => $kind:ident),* $(,)?) => {$(
        impl Described for $ty {
            const FIELDS: &'static [Field] = &[Field {
                name: Cow::Borrowed("value"),
                tag: 1,
                kind: Kind::$kind,
                cardinality: Cardinality::Required,
            }];
        }
    )*};
}

wrapper!(bool => Bool, u32 => U32, u64 => U64, i32 => I32, i64 => I64, f32 => F32, f64 => F64, String => String, Vec<u8> => Bytes);

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: Cow<'static, str>,
    pub tag: u32,
    pub kind: Kind,
    pub cardinality: Cardinality,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Bool,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    String,
    Bytes,
    Message { fields: Cow<'static, [Field]> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    Required,
    Optional,
    Repeated,
    Variant,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("stream schema exceeds 32 nested levels or 4096 total fields")]
    Budget,
    #[error("invalid schema field `{field}`: {reason}")]
    Field { field: String, reason: &'static str },
}

/* === Traits === */
pub trait Described: crate::Message {
    const FIELDS: &'static [Field];
}

/* === Implementations === */
impl Described for () {
    const FIELDS: &'static [Field] = &[];
}

/* === Public Functions === */
pub fn validate(fields: &[Field]) -> Result<(), Error> {
    let mut remaining = MAX_FIELDS;
    validate_fields(fields, 0, &mut remaining)
}

pub fn validation_memory_bytes(fields: &[Field]) -> Result<usize, Error> {
    let mut remaining = MAX_FIELDS;
    scratch_bytes(fields, 0, &mut remaining)?
        .checked_add(MAX_NAME_BYTES + std::mem::size_of::<Error>())
        .ok_or(Error::Budget)
}

/* === Private Functions === */
fn consume_fields(fields: &[Field], depth: usize, remaining: &mut usize) -> Result<(), Error> {
    if depth > MAX_DEPTH || fields.len() > *remaining {
        return Err(Error::Budget);
    }
    *remaining -= fields.len();
    Ok(())
}

fn scratch_bytes(fields: &[Field], depth: usize, remaining: &mut usize) -> Result<usize, Error> {
    consume_fields(fields, depth, remaining)?;
    let mut bytes = fields
        .len()
        .checked_mul(std::mem::size_of::<&str>() + std::mem::size_of::<u32>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<Vec<&str>>() + std::mem::size_of::<Vec<u32>>())
        })
        .ok_or(Error::Budget)?;
    for field in fields {
        if let Kind::Message { fields } = &field.kind {
            bytes = bytes
                .checked_add(scratch_bytes(fields, depth + 1, remaining)?)
                .ok_or(Error::Budget)?;
        }
    }
    Ok(bytes)
}

fn diagnostic_name(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_owned();
    }
    let mut end = MAX_NAME_BYTES - 3;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = String::with_capacity(MAX_NAME_BYTES);
    result.push_str(&name[..end]);
    result.push_str("...");
    result
}

fn validate_fields(fields: &[Field], depth: usize, remaining: &mut usize) -> Result<(), Error> {
    consume_fields(fields, depth, remaining)?;
    let mut names = Vec::with_capacity(fields.len());
    let mut tags = Vec::with_capacity(fields.len());
    let variants = fields
        .iter()
        .any(|field| field.cardinality == Cardinality::Variant);
    for field in fields {
        let invalid = |reason| Error::Field {
            field: diagnostic_name(&field.name),
            reason,
        };
        if variants
            && (field.cardinality != Cardinality::Variant
                || !matches!(field.kind, Kind::Message { .. }))
        {
            return Err(invalid(
                "an enum schema requires only message-valued variants",
            ));
        }
        if field.name.is_empty()
            || field.name.len() > MAX_NAME_BYTES
            || field.name.chars().any(char::is_control)
        {
            return Err(invalid(
                "name must be nonempty, at most 256 bytes, and contain no control characters",
            ));
        }
        match names.binary_search(&field.name.as_ref()) {
            Ok(_) => return Err(invalid("duplicate name")),
            Err(index) => names.insert(index, field.name.as_ref()),
        }
        if field.tag == 0 || field.tag > 536_870_911 || (19_000..=19_999).contains(&field.tag) {
            return Err(invalid("invalid protobuf tag"));
        }
        match tags.binary_search(&field.tag) {
            Ok(_) => return Err(invalid("duplicate protobuf tag")),
            Err(index) => tags.insert(index, field.tag),
        }
        if let Kind::Message { fields } = &field.kind {
            validate_fields(fields, depth + 1, remaining)?;
        }
    }
    Ok(())
}

/* === Tests === */
