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
// Local Imports
use crate::model::storage::{ObjectRetention, ObjectValue, SchemaKey, check_length, invalid, object_key};
use crate::{Error, Identified, Identifier, MemorySize, Result, schema};

/* === Constants === */

const MAX_DEFINITIONS: usize = 4096;
const MAX_IDENTITY_BYTES: usize = 1024;

const VALIDATION_BYTES: usize = 8 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug)]
pub struct ObjectDefinition {
    type_id: fn() -> std::any::TypeId,
    schema: fn() -> Result<SchemaKey>,
    declaration: schema::declaration::Definition,
    retention: ObjectRetention,
    verify: fn(&ObjectValue, NonZeroUsize) -> Result<()>,

    verify_admitted: fn(&ObjectValue, NonZeroUsize, usize) -> Result<()>,
}

#[derive(Clone, Debug, Default)]
pub struct ObjectCatalog {
    entries: Vec<ObjectDefinition>,
}

pub struct ObjectHandle<T: StoredObject> {
    definition: ObjectDefinition,
    marker: std::marker::PhantomData<fn() -> T>,
}

struct KeyMatch<'a> {
    expected: &'a str,
    offset: usize,
}

/* === Enums === */

/* === Traits === */

pub trait StoredObject: Identified + schema::Described + crate::Message + MemorySize + 'static {
    type Key: fmt::Display + ?Sized;

    const RETENTION: ObjectRetention;

    fn key(&self) -> &Self::Key;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/* === Implementations === */

impl ObjectCatalog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn entry(mut self, entry: ObjectDefinition) -> Self {
        self.entries.push(entry);
        self
    }
    pub fn extend(mut self, catalog: Self) -> Self {
        self.entries.extend(catalog.entries);
        self
    }
    pub fn build(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }
    pub fn definitions(&self) -> &[ObjectDefinition] {
        &self.entries
    }
    pub fn definition_count(&self) -> usize {
        self.entries.len()
    }
    pub fn handle<T: StoredObject>(&self) -> Result<ObjectHandle<T>> {
        self.validate()?;
        let definition = self
            .entries
            .iter()
            .find(|entry| (entry.type_id)() == std::any::TypeId::of::<T>())
            .copied()
            .ok_or_else(|| invalid("object type is absent from the catalog"))?;
        Ok(ObjectHandle {
            definition,
            marker: std::marker::PhantomData,
        })
    }
    pub fn encode<T: StoredObject>(&self, value: &T, max_payload_bytes: NonZeroUsize) -> Result<ObjectValue> {
        self.handle::<T>()?.encode(value, max_payload_bytes)
    }
    pub fn decode<T: StoredObject>(&self, value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<T> {
        self.handle::<T>()?.decode(value, max_payload_bytes)
    }
    pub fn validate(&self) -> Result<()> {
        let count = self.definition_count();
        if count > MAX_DEFINITIONS {
            return Err(invalid("object catalog exceeds its definition limit"));
        }
        let entries = self.definitions();
        let mut visited = 0usize;
        entries.iter().try_for_each(|_| {
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
            .and_then(|bytes| bytes.checked_add(MAX_IDENTITY_BYTES + std::mem::size_of::<Vec<(SchemaKey, ObjectRetention)>>()))
            .ok_or_else(|| invalid("object catalog validation size overflow"))?;
        if bytes > VALIDATION_BYTES {
            return Err(invalid("object catalog exceeds its validation byte limit"));
        }
        let mut identities = Vec::with_capacity(count);
        entries.iter().try_for_each(|entry| {
            if identities.len() >= count {
                return Err(invalid("object catalog projection count differs"));
            }
            let key = entry.schema()?;
            if key.id.as_str().len() > MAX_IDENTITY_BYTES {
                return Err(invalid("object schema identity exceeds its byte limit"));
            }
            let mut settings = crate::config::SchemaSettings::default();
            settings.preparation_bytes = NonZeroUsize::new(settings.preparation_bytes.get().min(VALIDATION_BYTES - bytes))
                .ok_or_else(|| invalid("object schema preparation exceeds allowance"))?;
            entry.graph(&settings)?;
            identities.push((key, entry.retention));
            Ok(())
        })?;
        if identities.len() != count {
            return Err(invalid("object catalog projection count differs"));
        }
        identities.sort_unstable_by(|(left, _), (right, _)| (left.id.as_str(), left.version).cmp(&(right.id.as_str(), right.version)));
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

    pub fn validate_value(&self, value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<()> {
        self.validate()?;
        check_length(value.payload.len(), max_payload_bytes)?;
        let mut verify = None;
        self.definitions().iter().try_for_each(|entry| {
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
impl<T: StoredObject> ObjectHandle<T> {
    pub fn definition(&self) -> ObjectDefinition {
        self.definition
    }
    pub fn encode(&self, value: &T, max_payload_bytes: NonZeroUsize) -> Result<ObjectValue> {
        value.validate()?;
        let schema = object_schema::<T>()?;
        let item = object_key(value.key(), max_payload_bytes)?;
        let length = value.encoded_size();
        check_length(length, max_payload_bytes)?;
        let payload = value.encode();
        if payload.len() != length {
            return Err(invalid("object codec reported an inconsistent length"));
        }
        Ok(ObjectValue { schema, item, payload })
    }

    pub fn decode(&self, value: &ObjectValue, max_payload_bytes: NonZeroUsize) -> Result<T> {
        decode::<T>(value, max_payload_bytes)
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

    pub fn graph(&self, settings: &crate::config::SchemaSettings) -> Result<schema::graph::Schema> {
        schema::graph::Schema::from_declaration(self.declaration, settings)
            .map_err(|_| invalid("invalid object schema or preparation allowance"))
    }

    pub fn validate_admitted(&self, value: &ObjectValue, payload: NonZeroUsize, allowance: usize) -> Result<()> {
        check_length(value.payload.len(), payload)?;
        check_length(value.item.as_str().len(), payload)?;
        (self.verify_admitted)(value, payload, allowance)
    }
}

impl ObjectDefinition {
    pub const fn of<T: StoredObject>() -> Self {
        Self {
            type_id: std::any::TypeId::of::<T>,
            schema: object_schema::<T>,
            declaration: T::DEFINITION,
            retention: T::RETENTION,
            verify: verify::<T>,
            verify_admitted: verify_admitted::<T>,
        }
    }
}

impl ObjectCatalog {
    pub fn definition(&self, key: &SchemaKey) -> Result<ObjectDefinition> {
        let mut found = None;
        self.definitions().iter().try_for_each(|entry| {
            if found.is_none() && entry.schema()? == *key {
                found = Some(*entry);
            }
            Ok::<(), Error>(())
        })?;
        found.ok_or_else(|| invalid("object schema is absent from the catalog"))
    }
    pub fn retention(&self, key: &SchemaKey) -> Result<ObjectRetention> {
        let mut retention = ObjectRetention::All;
        self.definitions().iter().try_for_each(|entry| {
            if entry.schema()? == *key {
                retention = entry.retention();
            }
            Ok::<(), Error>(())
        })?;
        Ok(retention)
    }
}

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

pub fn object_schema<T: StoredObject>() -> Result<SchemaKey> {
    let (id, version) = T::definition();
    if id.as_str().len() > MAX_IDENTITY_BYTES {
        return Err(invalid("object schema identity exceeds its byte limit"));
    }
    Ok(SchemaKey {
        payload_encoding: crate::model::storage::PayloadEncoding::PostcardV1,
        id: Identifier::new(id.as_str()).map_err(|_| invalid("invalid object schema identity"))?,
        version: NonZeroU32::new(version).ok_or_else(|| invalid("zero object schema version"))?,
    })
}

pub fn decode_admitted<T: StoredObject>(value: &ObjectValue, schema: &SchemaKey) -> Result<T> {
    if &value.schema != schema {
        return Err(invalid("object schema does not match the requested type"));
    }
    let decoded = T::decode(&value.payload).map_err(|error| Error::Corrupt {
        operation: "decode object",
        source: error,
    })?;
    decoded.validate()?;
    let mut key = KeyMatch { expected: value.item.as_str(), offset: 0 };
    fmt::write(&mut key, format_args!("{}", decoded.key())).map_err(|_| invalid("decoded object identity differs from stored key"))?;
    if key.offset != value.item.as_str().len() {
        return Err(invalid("decoded object identity differs from stored key"));
    }
    Ok(decoded)
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

fn verify_admitted<T: StoredObject>(value: &ObjectValue, payload: NonZeroUsize, allowance: usize) -> Result<()> {
    let overhead = std::mem::size_of::<crate::MessageError>() + MAX_IDENTITY_BYTES * 2;
    let native = allowance.checked_sub(overhead).ok_or_else(verification_limit)?;
    if std::mem::size_of::<T>() > native {
        return Err(verification_limit());
    }
    let required = T::decoding_memory(&value.payload).map_err(|error| Error::Corrupt {
        operation: "preflight object",
        source: error,
    })?;
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
    Error::ResourceExhausted { resource: "storage object verification" }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(Clone, PartialEq)]
    struct Document {
        id: String,
        value: u64,
    }
    #[derive(crate::Message, Clone, Debug, Default)]
    #[harmos_path(crate)]
    struct Other {
        id: String,
    }

    impl crate::Identified for Document {
        const ID: &'static str = "example.document";
    }
    impl crate::Identified for Other {
        const ID: &'static str = "example.other";
    }
    impl StoredObject for Document {
        type Key = str;
        const RETENTION: ObjectRetention = ObjectRetention::All;
        fn key(&self) -> &str {
            &self.id
        }
    }
    impl StoredObject for Other {
        type Key = str;
        const RETENTION: ObjectRetention = ObjectRetention::All;
        fn key(&self) -> &str {
            &self.id
        }
    }
    #[test]
    fn typed_handles_require_a_registered_entry() {
        let catalog = ObjectCatalog::new().entry(ObjectDefinition::of::<Document>()).build().unwrap();
        assert!(catalog.handle::<Other>().is_err());
        let handle = catalog.handle::<Document>().unwrap();
        let value = Document { id: "one".into(), value: 7 };
        let encoded = handle.encode(&value, NonZeroUsize::new(4096).unwrap()).unwrap();
        assert_eq!(handle.decode(&encoded, NonZeroUsize::new(4096).unwrap()).unwrap(), value);
        assert!(ObjectCatalog::new().entry(ObjectDefinition::of::<Document>()).entry(ObjectDefinition::of::<Document>()).build().is_err());
    }
}
