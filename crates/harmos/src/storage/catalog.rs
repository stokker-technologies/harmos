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
use std::fmt;
use std::num::{NonZeroU32, NonZeroUsize};
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::catalog::{Catalog, Contains, Member, Projection, Visitor};
use crate::{BackendFailure, Error, Identified, Identifier, MemorySize, Result, schema};

/* === Constants === */
const MAX_DEFINITIONS: usize = 4096;
const MAX_IDENTITY_BYTES: usize = 1024;

pub(crate) const LOOKUP_MEMORY_BYTES: usize = MAX_IDENTITY_BYTES * 2;
const VALIDATION_BYTES: usize = 8 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaKey {
    pub id: Identifier,
    pub version: NonZeroU32,
    #[serde(default)]
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

#[derive(Clone, Copy, Debug)]
pub struct ObjectDefinition {
    schema: fn() -> Result<SchemaKey>,
    declaration: schema::declaration::Definition,
    retention: ObjectRetention,
    verify: fn(&ObjectValue, NonZeroUsize) -> Result<()>,

    verify_admitted: fn(&ObjectValue, NonZeroUsize, usize) -> Result<()>,
}

pub struct ObjectDefinitions;

struct KeyMatch<'a> {
    expected: &'a str,
    offset: usize,
}

struct KeyWriter {
    value: String,
    limit: usize,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncoding {
    #[default]
    ProtobufV1 = 0,
    PostcardV1 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectRetention {
    /// Preserve every revision until the consumer durably releases its lifecycle.
    UntilReleased,
    Latest,
    Versions(NonZeroUsize),
}

/* === Traits === */
pub trait StoredObject:
    Identified + schema::Described + crate::Message + MemorySize + 'static
{
    type Key: fmt::Display + ?Sized;

    const RETENTION: ObjectRetention;

    fn key(&self) -> &Self::Key;
}

pub trait ObjectCatalog: Catalog<ObjectDefinitions> + Send + Sync + 'static {
    fn definitions() -> <Self as Catalog<ObjectDefinitions>>::Entries {
        <Self as Catalog<ObjectDefinitions>>::ENTRIES
    }

    fn definition_count() -> usize {
        <<Self as Catalog<ObjectDefinitions>>::Entries as Projection<ObjectDefinition>>::LEN
    }

    fn validate() -> Result<()> {
        let count = Self::definition_count();
        if count > MAX_DEFINITIONS {
            return Err(invalid("object catalog exceeds its definition limit"));
        }
        let entries = Self::definitions();
        let mut visited = 0usize;
        entries.try_for_each(|_| {
            visited += 1;
            if visited > count {
                return Err(invalid("object catalog projection count differs"));
            }
            Ok(())
        })?;
        if visited != count {
            return Err(invalid("object catalog projection count differs"));
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<(SchemaKey, ObjectRetention)>() + MAX_IDENTITY_BYTES)
            .and_then(|bytes| {
                bytes.checked_add(
                    MAX_IDENTITY_BYTES + std::mem::size_of::<Vec<(SchemaKey, ObjectRetention)>>(),
                )
            })
            .ok_or_else(|| invalid("object catalog validation size overflow"))?;
        if bytes > VALIDATION_BYTES {
            return Err(invalid("object catalog exceeds its validation byte limit"));
        }
        let mut identities = Vec::with_capacity(count);
        entries.try_for_each(|entry| {
            if identities.len() >= count {
                return Err(invalid("object catalog projection count differs"));
            }
            let key = entry.schema()?;
            if key.id.as_str().len() > MAX_IDENTITY_BYTES {
                return Err(invalid("object schema identity exceeds its byte limit"));
            }
            let mut settings = harmos_contract::config::SchemaSettings::default();
            settings.preparation_bytes = NonZeroUsize::new(
                settings
                    .preparation_bytes
                    .get()
                    .min(VALIDATION_BYTES - bytes),
            )
            .ok_or_else(|| invalid("object schema preparation exceeds allowance"))?;
            entry.graph(&settings)?;
            identities.push((key, entry.retention));
            Ok(())
        })?;
        if identities.len() != count {
            return Err(invalid("object catalog projection count differs"));
        }
        identities.sort_unstable_by(|(left, _), (right, _)| {
            (left.id.as_str(), left.version).cmp(&(right.id.as_str(), right.version))
        });
        for pair in identities.windows(2) {
            if pair[0].0.id == pair[1].0.id {
                if pair[0].1 != pair[1].1 {
                    return Err(invalid("codec versions disagree on object retention"));
                }
                if pair[0].0.version == pair[1].0.version {
                    return Err(invalid("duplicate object schema identity and version"));
                }
            }
        }
        Ok(())
    }

    fn encode<T: StoredObject>(value: &T, max_payload_bytes: NonZeroUsize) -> Result<ObjectValue>
    where
        Self: Contains<T>,
    {
        Self::validate()?;
        let schema = object_schema::<T>()?;
        let item = object_key(value.key(), max_payload_bytes)?;
        let length = value.encoded_len();
        check_length(length, max_payload_bytes)?;
        let payload = value.encode_to_vec();
        if payload.len() != length {
            return Err(invalid("object codec reported an inconsistent length"));
        }
        Ok(ObjectValue {
            schema,
            item,
            payload,
        })
    }

    fn decode<T: StoredObject>(value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<T>
    where
        Self: Contains<T>,
    {
        Self::validate()?;
        decode::<T>(value, max_payload_bytes)
    }

    fn validate_value(value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<()> {
        Self::validate()?;
        check_length(value.payload.len(), max_payload_bytes)?;
        let mut verify = None;
        Self::definitions().try_for_each(|entry| {
            if verify.is_none() && entry.schema()? == value.schema {
                verify = Some(entry.verify);
            }
            Ok::<(), Error>(())
        })?;
        match verify {
            Some(verify) => verify(value, max_payload_bytes),
            None => Err(invalid("object schema is absent from the catalog")),
        }
    }
}

/* === Implementations === */
impl SchemaKey {
    pub fn require_postcard(&self) -> Result<()> {
        if self.payload_encoding != PayloadEncoding::PostcardV1 {
            return Err(invalid(
                "payload requires an explicitly selected legacy decoder or migration",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "grpc")]
    pub fn migrate_protobuf<T: crate::Message + crate::__private::Protobuf + MemorySize>(
        &self,
        payload: &[u8],
        target: &Self,
        max_payload_bytes: NonZeroUsize,
        max_memory_bytes: NonZeroUsize,
    ) -> Result<Vec<u8>> {
        target.require_postcard()?;
        if self.payload_encoding != PayloadEncoding::ProtobufV1
            || self.id != target.id
            || self.version >= target.version
        {
            return Err(invalid(
                "migration requires legacy protobuf and a newer version of the same schema",
            ));
        }
        check_length(payload.len(), max_payload_bytes)?;
        let required = <T as crate::__private::Protobuf>::decoding_memory(payload)
            .map_err(|_| invalid("invalid legacy protobuf payload"))?;
        if required > max_memory_bytes.get() {
            return Err(invalid("migration exceeds its memory allowance"));
        }
        let value = <T as crate::__private::Protobuf>::decode(payload)
            .map_err(|_| invalid("invalid legacy protobuf payload"))?;
        let length = crate::Message::encoded_len(&value);
        check_length(length, max_payload_bytes)?;
        if value
            .memory_size()
            .checked_add(length)
            .is_none_or(|size| size > max_memory_bytes.get())
        {
            return Err(invalid("migration exceeds its memory allowance"));
        }
        Ok(crate::Message::encode_to_vec(&value))
    }
}

impl MemorySize for SchemaKey {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.id
                .memory_size()
                .saturating_sub(std::mem::size_of::<Identifier>()),
        )
    }
}

impl MemorySize for ObjectValue {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.schema
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<SchemaKey>()),
            )
            .saturating_add(
                self.item
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ObjectKey>()),
            )
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

    pub(crate) fn from_owned(value: String) -> Result<Self> {
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
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
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

impl ObjectDefinition {
    pub fn retention(&self) -> ObjectRetention {
        self.retention
    }

    pub fn schema(&self) -> Result<SchemaKey> {
        (self.schema)()
    }

    pub fn declaration(&self) -> schema::declaration::Definition {
        self.declaration
    }

    pub fn graph(
        &self,
        settings: &harmos_contract::config::SchemaSettings,
    ) -> Result<schema::graph::Schema> {
        schema::graph::Schema::from_declaration(self.declaration, settings)
            .map_err(|_| invalid("invalid object schema or preparation allowance"))
    }

    pub(crate) fn validate_admitted(
        &self,
        value: &ObjectValue,
        payload: NonZeroUsize,
        allowance: usize,
    ) -> Result<()> {
        check_length(value.payload.len(), payload)?;
        check_length(value.item.as_str().len(), payload)?;
        (self.verify_admitted)(value, payload, allowance)
    }
}

impl Visitor for ObjectDefinitions {
    type Entry = ObjectDefinition;
}

impl<T: StoredObject> Member<T> for ObjectDefinitions {
    const ENTRY: ObjectDefinition = ObjectDefinition {
        schema: object_schema::<T>,
        declaration: T::DEFINITION,
        retention: T::RETENTION,
        verify: verify::<T>,

        verify_admitted: verify_admitted::<T>,
    };
}

impl<C: Catalog<ObjectDefinitions> + Send + Sync + 'static> ObjectCatalog for C {}

impl fmt::Write for KeyMatch<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let rest = self.expected.get(self.offset..).ok_or(fmt::Error)?;
        if !rest.starts_with(value) {
            return Err(fmt::Error);
        }
        self.offset += value.len();
        Ok(())
    }
}

/* === Public Functions === */

pub(crate) fn definition<C: ObjectCatalog>(key: &SchemaKey) -> Result<ObjectDefinition> {
    let mut found = None;
    C::definitions().try_for_each(|entry| {
        if found.is_none() && entry.schema()? == *key {
            found = Some(*entry);
        }
        Ok::<(), Error>(())
    })?;
    found.ok_or_else(|| invalid("object schema is absent from the catalog"))
}

pub(crate) fn object_schema<T: StoredObject>() -> Result<SchemaKey> {
    let (id, version) = T::definition();
    if id.as_str().len() > MAX_IDENTITY_BYTES {
        return Err(invalid("object schema identity exceeds its byte limit"));
    }
    Ok(SchemaKey {
        payload_encoding: crate::storage::PayloadEncoding::PostcardV1,
        id: Identifier::new(id.as_str()).map_err(|_| invalid("invalid object schema identity"))?,
        version: NonZeroU32::new(version).ok_or_else(|| invalid("zero object schema version"))?,
    })
}

pub(crate) fn object_key<K: fmt::Display + ?Sized>(
    key: &K,
    limit: NonZeroUsize,
) -> Result<ObjectKey> {
    let mut writer = KeyWriter {
        value: String::new(),
        limit: limit.get(),
    };
    fmt::write(&mut writer, format_args!("{key}"))
        .map_err(|_| invalid("invalid or oversized object key"))?;
    ObjectKey::from_owned(writer.value)
}

pub(crate) fn decode_admitted<T: StoredObject>(
    value: &ObjectValue,
    schema: &SchemaKey,
) -> Result<T> {
    if &value.schema != schema {
        return Err(invalid("object schema does not match the requested type"));
    }
    let decoded = T::decode(&value.payload)
        .map_err(|error| Error::backend("decode object", BackendFailure::Corrupt, error))?;
    let mut key = KeyMatch {
        expected: value.item.as_str(),
        offset: 0,
    };
    fmt::write(&mut key, format_args!("{}", decoded.key()))
        .map_err(|_| invalid("decoded object identity differs from stored key"))?;
    if key.offset != value.item.as_str().len() {
        return Err(invalid("decoded object identity differs from stored key"));
    }
    Ok(decoded)
}

pub(crate) fn check_length(length: usize, max_payload_bytes: NonZeroUsize) -> Result<()> {
    if length > max_payload_bytes.get() {
        return Err(invalid("object payload exceeds byte limit"));
    }
    Ok(())
}

pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "object catalog",
        reason,
    }
}

/* === Private Functions === */
fn decode<T: StoredObject>(value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<T> {
    check_length(value.payload.len(), max_payload_bytes)?;
    check_length(value.item.as_str().len(), max_payload_bytes)?;
    decode_admitted::<T>(value, &object_schema::<T>()?)
}

fn verify<T: StoredObject>(value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<()> {
    decode::<T>(value, max_payload_bytes).map(|_| ())
}

fn verify_admitted<T: StoredObject>(
    value: &ObjectValue,
    payload: NonZeroUsize,
    allowance: usize,
) -> Result<()> {
    let overhead = std::mem::size_of::<crate::MessageError>() + MAX_IDENTITY_BYTES * 2;
    let native = allowance
        .checked_sub(overhead)
        .ok_or_else(verification_limit)?;
    if std::mem::size_of::<T>() > native {
        return Err(verification_limit());
    }
    let required = T::decoding_memory(&value.payload)
        .map_err(|error| Error::backend("preflight object", BackendFailure::Corrupt, error))?;
    if required > native {
        return Err(verification_limit());
    }
    let decoded = decode::<T>(value, payload)?;
    if decoded.memory_size() > required {
        return Err(verification_limit());
    }
    Ok(())
}

fn verification_limit() -> Error {
    Error::ResourceExhausted {
        resource: "storage object verification",
    }
}

/* === Tests === */
