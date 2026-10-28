/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Schema, Object And Payload Keys Shared By Storage Requests And Records.

use std::fmt;
use std::num::{NonZeroU32, NonZeroUsize};

use serde::{Deserialize, Serialize};

use crate::{Error, Identifier, MemorySize, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaKey {
    pub id: Identifier,
    pub version: NonZeroU32,

    pub payload_encoding: PayloadEncoding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectValue {
    pub schema: SchemaKey,
    pub item: ObjectKey,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ObjectKey(String);

struct KeyWriter {
    value: String,
    limit: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncoding {
    #[default]
    PostcardV1 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectRetention {
    UntilReleased,

    Latest,
    Versions(NonZeroUsize),
}

impl MemorySize for SchemaKey {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.id.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
    }
}

impl MemorySize for ObjectValue {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of::<SchemaKey>()))
            .saturating_add(self.item.memory_size().saturating_sub(std::mem::size_of::<ObjectKey>()))
            .saturating_add(self.payload.capacity())
    }
}

impl MemorySize for ObjectKey {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl ObjectKey {
    pub fn new<K: fmt::Display + ?Sized>(key: &K, max_bytes: NonZeroUsize) -> Result<Self> {
        object_key(key, max_bytes)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_owned(value: String) -> Result<Self> {
        if value.is_empty() {
            return Err(invalid("object key is empty"));
        }

        Ok(Self(value))
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ObjectKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::from_owned(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Write for KeyWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.limit - self.value.len() {
            return Err(fmt::Error);
        }
        let remaining = self.value.capacity() - self.value.len();
        if value.len() > remaining {
            self.value.reserve_exact(value.len());
        }
        self.value.push_str(value);
        Ok(())
    }
}

impl ObjectRetention {
    pub const fn versions(self) -> NonZeroUsize {
        match self {
            Self::UntilReleased | Self::Latest => NonZeroUsize::MIN,
            Self::Versions(count) => count,
        }
    }
}

pub fn object_key<K: fmt::Display + ?Sized>(key: &K, limit: NonZeroUsize) -> Result<ObjectKey> {
    let mut writer = KeyWriter { value: String::new(), limit: limit.get() };
    fmt::write(&mut writer, format_args!("{key}")).map_err(|_| invalid("invalid or oversized object key"))?;
    ObjectKey::from_owned(writer.value)
}

pub fn check_length(length: usize, max_payload_bytes: NonZeroUsize) -> Result<()> {
    if length > max_payload_bytes.get() {
        return Err(invalid("object payload exceeds byte limit"));
    }
    Ok(())
}

pub fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "object catalog", reason }
}
