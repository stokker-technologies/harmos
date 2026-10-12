/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod codec;
pub(crate) mod optional_datetime {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer
                .serialize_some(&datetime_text(*value).map_err(serde::ser::Error::custom)?),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                OffsetDateTime::parse(&value, &Rfc3339)
                    .map(|value| value.to_offset(UtcOffset::UTC))
                    .map_err(D::Error::custom)
            })
            .transpose()
    }
}

/* === Re-Exports === */
pub use codec::VALUE_DEPTH;

/* === Imports === */
// Standard Library Imports
use std::{collections::BTreeMap, fmt};
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
// Local Imports
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct DisplayText<'a, T>(&'a T);

struct NumberText(f64);

struct DatetimeText {
    bytes: [u8; 30],
    length: usize,
}

/* === Enums === */
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Number(f64),
    Text(String),
    Datetime(OffsetDateTime),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

#[derive(Debug, thiserror::Error)]
pub enum ValueError {
    #[error("non-finite numbers have no plain JSON representation")]
    NonFinite,
    #[error("JSON number cannot be represented without loss")]
    InvalidNumber,
    #[error("datetime cannot be formatted as RFC3339: {0}")]
    Datetime(#[from] time::error::Format),
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Representation {
    Null,
    Bool(bool),
    Integer(String),
    Unsigned(String),
    Number(String),
    Text(String),
    Datetime(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    rename = "Representation"
)]
enum BorrowedRepresentation<'a> {
    Null,
    Bool(bool),
    Integer(DisplayText<'a, i64>),
    Unsigned(DisplayText<'a, u64>),
    Number(NumberText),
    Text(&'a str),
    Datetime(DatetimeText),
    Array(&'a [Value]),
    Object(&'a BTreeMap<String, Value>),
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for Value {
    fn memory_size(&self) -> usize {
        let allocation = match self {
            Self::Text(value) => value.capacity(),
            Self::Array(value) => {
                let bytes = value.memory_size();
                if bytes == usize::MAX {
                    return usize::MAX;
                }
                bytes.saturating_sub(std::mem::size_of_val(value))
            }
            Self::Object(value) => {
                let bytes = value.memory_size();
                if bytes == usize::MAX {
                    return usize::MAX;
                }
                bytes.saturating_sub(std::mem::size_of_val(value))
            }
            _ => 0,
        };
        std::mem::size_of::<Self>().saturating_add(allocation)
    }
}

impl Value {
    pub fn from_plain_json(value: serde_json::Value) -> Result<Self, ValueError> {
        Ok(match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Bool(value),
            serde_json::Value::String(value) => Self::Text(value),
            serde_json::Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    Self::Integer(value)
                } else if let Some(value) = value.as_u64() {
                    Self::Unsigned(value)
                } else {
                    Self::Number(value.as_f64().ok_or(ValueError::InvalidNumber)?)
                }
            }
            serde_json::Value::Array(values) => Self::Array(
                values
                    .into_iter()
                    .map(Self::from_plain_json)
                    .collect::<Result<_, _>>()?,
            ),
            serde_json::Value::Object(values) => Self::Object(
                values
                    .into_iter()
                    .map(|(key, value)| Ok((key, Self::from_plain_json(value)?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
        })
    }

    pub fn to_plain_json(&self) -> Result<serde_json::Value, ValueError> {
        Ok(match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => (*value).into(),
            Self::Integer(value) => (*value).into(),
            Self::Unsigned(value) => (*value).into(),
            Self::Number(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(*value).ok_or(ValueError::NonFinite)?,
            ),
            Self::Text(value) => value.clone().into(),
            Self::Datetime(value) => format_datetime(*value)?.into(),
            Self::Array(values) => serde_json::Value::Array(
                values
                    .iter()
                    .map(Self::to_plain_json)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Object(values) => serde_json::Value::Object(
                values
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), value.to_plain_json()?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
        })
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Unsigned(a), Self::Unsigned(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => {
                (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
            }
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Datetime(a), Self::Datetime(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let representation = match self {
            Self::Null => BorrowedRepresentation::Null,
            Self::Bool(value) => BorrowedRepresentation::Bool(*value),
            Self::Integer(value) => BorrowedRepresentation::Integer(DisplayText(value)),
            Self::Unsigned(value) => BorrowedRepresentation::Unsigned(DisplayText(value)),
            Self::Number(value) => BorrowedRepresentation::Number(NumberText(*value)),
            Self::Text(value) => BorrowedRepresentation::Text(value),
            Self::Datetime(value) => BorrowedRepresentation::Datetime(
                datetime_text(*value).map_err(serde::ser::Error::custom)?,
            ),
            Self::Array(value) => BorrowedRepresentation::Array(value),
            Self::Object(value) => BorrowedRepresentation::Object(value),
        };
        serializer.serialize_newtype_struct("harmos::Value", &representation)
    }
}

impl<T: fmt::Display> Serialize for DisplayText<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.0)
    }
}

impl Serialize for NumberText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_nan() {
            serializer.serialize_str("nan")
        } else {
            serializer.collect_str(&self.0)
        }
    }
}

impl Serialize for DatetimeText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text =
            std::str::from_utf8(&self.bytes[..self.length]).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl std::io::Write for DatetimeText {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.bytes.len() - self.length {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        let end = self.length + bytes.len();
        self.bytes[self.length..end].copy_from_slice(bytes);
        self.length = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canonical tagged Harmos value")
            }

            fn visit_newtype_struct<D: Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Value, D::Error> {
                Ok(match Representation::deserialize(deserializer)? {
                    Representation::Null => Value::Null,
                    Representation::Bool(value) => Value::Bool(value),
                    Representation::Integer(value) => {
                        Value::Integer(value.parse().map_err(D::Error::custom)?)
                    }
                    Representation::Unsigned(value) => {
                        Value::Unsigned(value.parse().map_err(D::Error::custom)?)
                    }
                    Representation::Number(value) => {
                        Value::Number(value.parse().map_err(D::Error::custom)?)
                    }
                    Representation::Text(value) => Value::Text(value),
                    Representation::Datetime(value) => Value::Datetime(
                        OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(D::Error::custom)?
                            .to_offset(UtcOffset::UTC),
                    ),
                    Representation::Array(value) => Value::Array(value),
                    Representation::Object(value) => Value::Object(value),
                })
            }
        }

        deserializer.deserialize_newtype_struct("harmos::Value", ValueVisitor)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Unsigned(value.into())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<OffsetDateTime> for Value {
    fn from(value: OffsetDateTime) -> Self {
        Self::Datetime(value.to_offset(UtcOffset::UTC))
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(value: Vec<T>) -> Self {
        Self::Array(value.into_iter().map(Into::into).collect())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn datetime_text(value: OffsetDateTime) -> Result<DatetimeText, time::error::Format> {
    // UTC RFC3339 Uses At Most 30 Bytes, Including All Nine Fractional Digits.
    let mut text = DatetimeText {
        bytes: [0; 30],
        length: 0,
    };
    value
        .to_offset(UtcOffset::UTC)
        .format_into(&mut text, &Rfc3339)?;
    Ok(text)
}

fn format_datetime(value: OffsetDateTime) -> Result<String, time::error::Format> {
    value.to_offset(UtcOffset::UTC).format(&Rfc3339)
}

/* === Tests === */
