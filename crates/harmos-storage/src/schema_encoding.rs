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
use serde::de::{DeserializeSeed, Error as _, IntoDeserializer, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports
use super::manager::encoded_size;
use super::{SchemaKey, StorageCursor, StorageSchema};
use crate::schema::graph::Schema;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Input<A> {
    map: A,
    position: Option<StorageCursor>,
    key: Option<SchemaKey>,
    encoding: Option<Encoding>,
}

struct StorageVisitor;

/* === Enums === */
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Encoding {
    GraphV1,
}

/* === Traits === */

/* === Implementations === */
impl StorageSchema {
    // Empty definitions have the highest allocation-to-JSON ratio: each `[]`
    // retains a vector header and traversal state. Include vector growth and the
    // bounded field-name diagnostic buffer before asking the backend for a page.
    pub fn decoding_memory_bytes(encoded_bytes: usize) -> Option<usize> {
        encoded_bytes.checked_mul(36)?.checked_add(
            4 * std::mem::size_of::<Schema>()
                + 4 * std::mem::size_of::<Vec<harmos_domain::schema::graph::Field>>()
                + harmos_domain::schema::MAX_NAME_BYTES,
        )
    }

    pub(super) fn encoded_bytes(&self) -> Result<usize> {
        self.schema.preparation(&harmos_domain::config::SchemaSettings::default()).map_err(|_| invalid())?;
        encoded_size(self)
    }

    pub fn encoding_memory_bytes(&self) -> Result<usize> {
        self.schema.validation_memory_bytes(&harmos_domain::config::SchemaSettings::default()).map_err(|_| invalid())
    }
}

impl Serialize for StorageSchema {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("StorageSchema", 5)?;
        output.serialize_field("position", &self.position)?;
        output.serialize_field("key", &self.key)?;
        output.serialize_field("encoding", "graph_v1")?;
        output.serialize_field("root", &self.schema.root)?;
        output.serialize_field("definitions", &self.schema.definitions)?;
        output.end()
    }
}

impl<'de> Deserialize<'de> for StorageSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_map(StorageVisitor).map_err(|error| D::Error::custom(format!(
            "invalid graph_v1 storage schema: {error}; inline metadata is unsupported; retain the original data and select a fresh storage root",
        )))
    }
}

impl<'de> Visitor<'de> for StorageVisitor {
    type Value = StorageSchema;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("bounded graph_v1 storage metadata")
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> std::result::Result<Self::Value, A::Error> {
        let mut input = Input { map, position: None, key: None, encoding: None };
        // Forward Only The Graph Fields To The Canonical Seed Without Buffering
        // The JSON Object Or Allocating A Second Representation Of Its Fields.
        let schema = crate::schema::graph::Seed { settings: &harmos_domain::config::SchemaSettings::default() }
            .deserialize(serde::de::value::MapAccessDeserializer::new(&mut input))?;
        let Encoding::GraphV1 = input.encoding.ok_or_else(|| A::Error::missing_field("encoding"))?;
        Ok(StorageSchema::new(
            input.position.ok_or_else(|| A::Error::missing_field("position"))?,
            input.key.ok_or_else(|| A::Error::missing_field("key"))?,
            schema,
        ))
    }
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Input<A> {
    type Error = A::Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> std::result::Result<Option<K::Value>, A::Error> {
        while let Some(key) = self.map.next_key::<String>()? {
            match key.as_str() {
                "position" => {
                    if self.position.is_some() {
                        return Err(A::Error::duplicate_field("position"));
                    }
                    self.position = Some(self.map.next_value()?);
                }
                "key" => {
                    if self.key.is_some() {
                        return Err(A::Error::duplicate_field("key"));
                    }
                    self.key = Some(self.map.next_value()?);
                }
                "encoding" => {
                    if self.encoding.is_some() {
                        return Err(A::Error::duplicate_field("encoding"));
                    }
                    self.encoding = Some(self.map.next_value()?);
                }
                _ => return seed.deserialize(key.into_deserializer()).map(Some),
            }
        }
        Ok(None)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> std::result::Result<V::Value, A::Error> {
        self.map.next_value_seed(seed)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid() -> Error {
    Error::InvalidInput { field: "storage schema", reason: "invalid shared schema preparation" }
}

/* === Tests === */
