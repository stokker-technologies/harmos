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

use super::{SchemaKey, StorageCursor, StorageSchema};
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
    pub fn encoded_bytes(&self) -> Result<usize> {
        self.schema.preparation(&crate::config::SchemaSettings::default()).map_err(|_| invalid())?;
        struct Counter(usize);
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self.0.checked_add(bytes.len()).ok_or_else(|| std::io::Error::other("measure schema size: overflow"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut counter = Counter(0);
        serde_json::to_writer(&mut counter, self).map_err(|_| invalid())?;
        Ok(counter.0)
    }
}

impl Serialize for StorageSchema {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("StorageSchema", 5 + usize::from(!self.schema.representations.is_empty()))?;
        output.serialize_field("position", &self.position)?;
        output.serialize_field("key", &self.key)?;
        output.serialize_field("encoding", "graph_v1")?;
        output.serialize_field("root", &self.schema.root)?;
        output.serialize_field("definitions", &self.schema.definitions)?;
        if !self.schema.representations.is_empty() {
            // Use The Graph's Bounded Representation Encoding For Stored Definitions.
            struct Representations<'a>(&'a crate::schema::graph::Schema);
            impl Serialize for Representations<'_> {
                fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                    crate::schema::wire::serialize(&self.0.representations, serializer)
                }
            }
            output.serialize_field("representations", &Representations(&self.schema))?;
        }
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
        let mut input = Input {
            map,
            position: None,
            key: None,
            encoding: None,
        };
        // Forward Only the Graph Fields to the Canonical Seed Without Buffering
        // The JSON Object Or Allocating A Second Representation Of Its Fields.
        let schema = crate::schema::graph::Seed {
            settings: &crate::config::SchemaSettings::default(),
        }
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
    Error::InvalidInput {
        field: "storage schema",
        reason: "invalid shared schema preparation",
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;
    use crate::config::SchemaSettings;
    use crate::model::storage::{PayloadEncoding, StoreHistory};
    use crate::schema::{Described as SchemaDescribed, graph::Schema, identity, json::JsonCodec};
    use crate::{Described, Identifier};

    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Stored {
        Value { value: u32 },
    }

    #[test]
    fn stored_schema_retains_serde_representation() {
        let schema = Schema::from_declaration(<Stored as SchemaDescribed>::DEFINITION, &SchemaSettings::default()).unwrap();
        let revision = identity::revision(&schema).unwrap();
        let position = StorageCursor {
            history: StoreHistory {
                store: Identifier::new("store").unwrap(),
                epoch: Identifier::new("epoch").unwrap(),
            },
            sequence: 1,
        };
        let key = SchemaKey {
            id: Identifier::new("record").unwrap(),
            version: NonZeroU32::new(revision).unwrap(),
            payload_encoding: PayloadEncoding::PostcardV1,
        };
        let stored = StorageSchema::new(position, key, schema);
        let bytes = serde_json::to_vec(&stored).unwrap();
        let decoded: StorageSchema = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, stored);
        assert_eq!(identity::revision(&decoded.schema).unwrap(), revision);
        let value = serde_json::to_value(Stored::Value { value: 7 }).unwrap();
        let codec = JsonCodec::new(decoded.schema.root().unwrap(), &SchemaSettings::default()).unwrap();
        let payload = codec.encode(&value).unwrap();
        assert_eq!(codec.decode(&payload).unwrap(), value);
    }
}
