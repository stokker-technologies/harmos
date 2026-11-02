/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Message Schema Definitions And References

/* === Modules === */

mod codec;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::mem::size_of;
// Third Party Imports
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
// Local Imports
use super::{Cardinality, MAX_NAME_BYTES, diagnostic_name};
use crate::config::SchemaSettings;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub root: usize,
    pub definitions: Vec<Vec<Field>>,
}

#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    schema: &'a Schema,
    root: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field<R = usize> {
    #[serde(deserialize_with = "name")]
    pub name: Cow<'static, str>,

    pub tag: u32,
    pub kind: Kind<R>,
    pub cardinality: Cardinality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preparation {
    pub definitions: usize,
    pub fields: usize,
    pub bytes: usize,
    pub work: usize,
}

pub struct Seed<'a> {
    pub settings: &'a SchemaSettings,
}

struct DefinitionsSeed<'a>(&'a mut Budget);
struct DefinitionSeed<'a>(&'a mut Budget);
struct FieldSeed<'a>(&'a mut Budget);
struct Budget {
    remaining: usize,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind<R = usize> {
    Bool,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    String,
    Bytes,
    Message { definition: R },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("schema preparation exceeds its memory or work allowance")]
    Budget,

    #[error("schema preparation {budget} budget exceeded: used {used}, allowed {allowed}")]
    Allowance { budget: &'static str, used: usize, allowed: usize },

    #[error("schema definition {definition} is absent")]
    Reference { definition: usize },

    #[error("schema definition {definition} is unreachable")]
    Unreachable { definition: usize },

    #[error("invalid schema field `{field}`: {reason}")]
    Field { field: String, reason: &'static str },
}

/* === Traits === */

/* === Implementations === */

impl<R> Kind<R> {
    pub(crate) fn try_map<S, E>(self, reference: impl FnOnce(R) -> Result<S, E>) -> Result<Kind<S>, E> {
        Ok(match self {
            Self::Bool => Kind::Bool,
            Self::U32 => Kind::U32,
            Self::U64 => Kind::U64,
            Self::I32 => Kind::I32,
            Self::I64 => Kind::I64,
            Self::F32 => Kind::F32,
            Self::F64 => Kind::F64,
            Self::String => Kind::String,
            Self::Bytes => Kind::Bytes,
            Self::Message { definition } => Kind::Message { definition: reference(definition)? },
        })
    }
}

impl<'de, R: Deserialize<'de>> Deserialize<'de> for Kind<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, bound(deserialize = "R: Deserialize<'de>"))]
        struct Input<R> {
            #[serde(rename = "type", deserialize_with = "kind_name")]
            kind: String,

            #[serde(default, deserialize_with = "definition")]
            definition: Option<R>,
        }
        let input = Input::<R>::deserialize(deserializer)?;
        if input.kind == "message" {
            return input.definition.map(|definition| Self::Message { definition }).ok_or_else(|| D::Error::missing_field("definition"));
        }
        if input.definition.is_some() {
            return Err(D::Error::custom("scalar schema kind cannot reference a definition"));
        }
        match input.kind.as_str() {
            "bool" => Ok(Self::Bool),
            "u32" => Ok(Self::U32),
            "u64" => Ok(Self::U64),
            "i32" => Ok(Self::I32),
            "i64" => Ok(Self::I64),
            "f32" => Ok(Self::F32),
            "f64" => Ok(Self::F64),
            "string" => Ok(Self::String),
            "bytes" => Ok(Self::Bytes),
            _ => Err(D::Error::custom("unknown schema kind")),
        }
    }
}

impl crate::MemorySize for Schema {
    fn memory_size(&self) -> usize {
        let mut bytes = size_of::<Self>().saturating_add(self.definitions.capacity().saturating_mul(size_of::<Vec<Field>>()));
        for fields in &self.definitions {
            bytes = bytes.saturating_add(fields.capacity().saturating_mul(size_of::<Field>()));
            for field in fields {
                if let Cow::Owned(name) = &field.name {
                    bytes = bytes.saturating_add(name.capacity());
                }
            }
        }
        bytes
    }
}

impl Schema {
    pub fn view(&self, root: usize) -> Result<View<'_>, Error> {
        if root >= self.definitions.len() {
            return Err(Error::Reference { definition: root });
        }
        Ok(View { schema: self, root })
    }

    pub fn root(&self) -> Result<View<'_>, Error> {
        self.view(self.root)
    }

    // Compare Wire Shapes Independently Of Local Definition Indices.
    pub fn equivalent(&self, root: usize, other: &Self, other_root: usize, settings: &SchemaSettings) -> Result<bool, Error> {
        let mut preparation = self.prepare(settings)?;
        let right = other.prepare(settings)?;
        if root >= self.definitions.len() {
            return Err(Error::Reference { definition: root });
        }
        if other_root >= other.definitions.len() {
            return Err(Error::Reference { definition: other_root });
        }
        if !std::ptr::eq(self, other) {
            preparation.add_bytes(right.bytes, 1, settings)?;
            preparation.add_work(right.work, settings)?;
        }
        preparation.add_bytes(4, size_of::<(usize, usize)>(), settings)?;
        let mut pending = vec![(root, other_root)];
        let mut visited = HashSet::new();
        while let Some((left, right)) = pending.pop() {
            preparation.add_work(1, settings)?;
            if visited.contains(&(left, right)) {
                continue;
            }
            preparation.add_bytes(4, size_of::<(usize, usize)>(), settings)?;
            visited.insert((left, right));
            let left = &self.definitions[left];
            let right = &other.definitions[right];
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left, right) in left.iter().zip(right) {
                preparation.add_work(left.name.len() + 1, settings)?;
                if left.name != right.name || left.tag != right.tag || left.cardinality != right.cardinality {
                    return Ok(false);
                }
                match (left.kind, right.kind) {
                    (Kind::Message { definition: left }, Kind::Message { definition: right }) => {
                        preparation.add_bytes(4, size_of::<(usize, usize)>(), settings)?;
                        pending.push((left, right));
                    }
                    (left, right) if left == right => {}
                    _ => return Ok(false),
                }
            }
        }
        Ok(true)
    }

    pub fn preparation(&self, settings: &SchemaSettings) -> Result<Preparation, Error> {
        if self.root >= self.definitions.len() {
            return Err(Error::Reference { definition: self.root });
        }
        let mut preparation = Preparation {
            definitions: self.definitions.len(),
            fields: 0,
            bytes: size_of::<Self>(),
            work: 0,
        };
        // Charge Retained Storage And Traversal Scratch Before Allocating Scratch.
        preparation.add_bytes(self.definitions.capacity(), size_of::<Vec<Field>>(), settings)?;
        preparation.add_bytes(self.definitions.len(), size_of::<bool>() + size_of::<usize>(), settings)?;
        for fields in &self.definitions {
            preparation.fields = preparation.fields.checked_add(fields.len()).ok_or(Error::Budget)?;
            preparation.add_bytes(fields.capacity(), size_of::<Field>(), settings)?;
            preparation.add_work(fields.len() + 1, settings)?;
            for field in fields {
                if let Cow::Owned(name) = &field.name {
                    preparation.add_bytes(name.capacity(), 1, settings)?;
                }
                preparation.add_work(field.name.len(), settings)?;
            }
        }
        let scratch = self.definitions.iter().map(|fields| fields.len()).max().unwrap_or(0);
        preparation.add_bytes(scratch, size_of::<&str>() + size_of::<u32>(), settings)?;
        for fields in &self.definitions {
            let bits = usize::BITS as usize - fields.len().leading_zeros() as usize;
            let work = fields.len().checked_mul(2 * bits + 2).ok_or(Error::Budget)?;
            preparation.add_work(work, settings)?;
        }
        preparation.add_work(self.definitions.len(), settings)?;
        preparation.add_work(preparation.fields, settings)?;
        Ok(preparation)
    }

    pub fn prepare(&self, settings: &SchemaSettings) -> Result<Preparation, Error> {
        let preparation = self.preparation(settings)?;
        for fields in &self.definitions {
            // Sorting Is Bounded By Direct Fields, Independent Of Repeated References.
            let mut names = Vec::with_capacity(fields.len());
            let mut tags = Vec::with_capacity(fields.len());
            let variants = fields.iter().any(|field| field.cardinality == Cardinality::Variant);
            for field in fields {
                let invalid = |reason| Error::Field {
                    field: diagnostic_name(&field.name),
                    reason,
                };
                if variants && (field.cardinality != Cardinality::Variant || !matches!(field.kind, Kind::Message { .. })) {
                    return Err(invalid("an enum schema requires only message-valued variants"));
                }
                if field.name.is_empty() || field.name.len() > MAX_NAME_BYTES || field.name.chars().any(char::is_control) {
                    return Err(invalid("name must be nonempty, at most 256 bytes, and contain no control characters"));
                }
                if field.tag == 0 || field.tag > 536_870_911 || (19_000..=19_999).contains(&field.tag) {
                    return Err(invalid("invalid protobuf tag"));
                }
                if let Kind::Message { definition } = field.kind
                    && definition >= self.definitions.len()
                {
                    return Err(Error::Reference { definition });
                }
                names.push(field.name.as_ref());
                tags.push(field.tag);
            }
            names.sort_unstable();
            tags.sort_unstable();
            if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
                return Err(Error::Field {
                    field: diagnostic_name(pair[0]),
                    reason: "duplicate name",
                });
            }
            if let Some(pair) = tags.windows(2).find(|pair| pair[0] == pair[1]) {
                let field = fields.iter().find(|field| field.tag == pair[0]).unwrap();
                return Err(Error::Field {
                    field: diagnostic_name(&field.name),
                    reason: "duplicate protobuf tag",
                });
            }
        }
        let mut reached = vec![false; self.definitions.len()];
        let mut pending = Vec::with_capacity(self.definitions.len());
        reached[self.root] = true;
        pending.push(self.root);
        while let Some(definition) = pending.pop() {
            for field in &self.definitions[definition] {
                if let Kind::Message { definition } = field.kind
                    && !reached[definition]
                {
                    reached[definition] = true;
                    pending.push(definition);
                }
            }
        }
        if let Some(definition) = reached.iter().position(|reached| !reached) {
            return Err(Error::Unreachable { definition });
        }
        Ok(preparation)
    }
}

impl<'a> View<'a> {
    pub fn schema(self) -> &'a Schema {
        self.schema
    }
    pub fn index(self) -> usize {
        self.root
    }
    pub fn fields(self) -> &'a [Field] {
        &self.schema.definitions[self.root]
    }

    pub fn message(self, field: &Field) -> Result<Option<Self>, Error> {
        match field.kind {
            Kind::Message { definition } => self.schema.view(definition).map(Some),
            _ => Ok(None),
        }
    }
}

impl Preparation {
    fn add_bytes(&mut self, count: usize, size: usize, settings: &SchemaSettings) -> Result<(), Error> {
        self.bytes = count.checked_mul(size).and_then(|bytes| self.bytes.checked_add(bytes)).ok_or(Error::Budget)?;
        if self.bytes > settings.preparation_bytes.get() {
            return Err(Error::Allowance {
                budget: "memory bytes",
                used: self.bytes,
                allowed: settings.preparation_bytes.get(),
            });
        }
        Ok(())
    }

    fn add_work(&mut self, work: usize, settings: &SchemaSettings) -> Result<(), Error> {
        self.work = self.work.checked_add(work).ok_or(Error::Budget)?;
        if self.work > settings.preparation_work.get() {
            return Err(Error::Allowance {
                budget: "work",
                used: self.work,
                allowed: settings.preparation_work.get(),
            });
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Schema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Seed { settings: &SchemaSettings::default() }.deserialize(deserializer)
    }
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Schema;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Schema, D::Error> {
        struct Input<'a>(&'a SchemaSettings);
        impl<'de> Visitor<'de> for Input<'_> {
            type Value = Schema;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bounded shared message schema")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Schema, A::Error> {
                let mut root = None;
                let mut definitions = None;
                let mut budget = Budget {
                    remaining: self.0.preparation_bytes.get(),
                };
                while let Some(key) = input.next_key::<String>()? {
                    match key.as_str() {
                        "root" => {
                            if root.is_some() {
                                return Err(A::Error::duplicate_field("root"));
                            }
                            root = Some(input.next_value()?);
                        }
                        "definitions" => {
                            if definitions.is_some() {
                                return Err(A::Error::duplicate_field("definitions"));
                            }
                            definitions = Some(input.next_value_seed(DefinitionsSeed(&mut budget))?);
                        }
                        _ => return Err(A::Error::unknown_field(&key, &["root", "definitions"])),
                    }
                }
                let schema = Schema {
                    root: root.ok_or_else(|| A::Error::missing_field("root"))?,
                    definitions: definitions.ok_or_else(|| A::Error::missing_field("definitions"))?,
                };
                schema.prepare(self.0).map_err(A::Error::custom)?;
                Ok(schema)
            }
        }
        deserializer.deserialize_map(Input(self.settings))
    }
}

impl Budget {
    fn charge<E: serde::de::Error>(&mut self, bytes: usize) -> Result<(), E> {
        self.remaining = self.remaining.checked_sub(bytes).ok_or_else(|| E::custom(Error::Budget))?;
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for DefinitionsSeed<'_> {
    type Value = Vec<Vec<Field>>;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> DeserializeSeed<'de> for DefinitionSeed<'_> {
    type Value = Vec<Field>;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> DeserializeSeed<'de> for FieldSeed<'_> {
    type Value = Field;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        // Reserve Capacity And Sorting Scratch Before Any Field Allocation.
        self.0.charge::<D::Error>(4 * size_of::<Field>() + size_of::<&str>() + size_of::<u32>() + MAX_NAME_BYTES)?;
        Field::deserialize(deserializer)
    }
}

impl<'de> Visitor<'de> for DefinitionsSeed<'_> {
    type Value = Vec<Vec<Field>>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bounded message definitions")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
        let mut definitions = Vec::new();
        while let Some(fields) = input.next_element_seed(DefinitionSeed(self.0))? {
            definitions.push(fields);
        }
        Ok(definitions)
    }
}

impl<'de> Visitor<'de> for DefinitionSeed<'_> {
    type Value = Vec<Field>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bounded direct message fields")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
        self.0.charge::<A::Error>(4 * size_of::<Vec<Field>>() + size_of::<usize>() + size_of::<bool>())?;
        let mut fields = Vec::new();
        while let Some(field) = input.next_element_seed(FieldSeed(self.0))? {
            fields.push(field);
        }
        Ok(fields)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn definition<'de, D: Deserializer<'de>, R: Deserialize<'de>>(deserializer: D) -> Result<Option<R>, D::Error> {
    R::deserialize(deserializer).map(Some)
}

fn kind_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    name(deserializer).map(Cow::into_owned)
}

fn name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Cow<'static, str>, D::Error> {
    struct Name;
    impl<'de> Visitor<'de> for Name {
        type Value = String;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a field name of at most 256 bytes")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<String, E> {
            if value.len() > MAX_NAME_BYTES {
                return Err(E::custom("schema field name exceeds 256 bytes"));
            }
            Ok(value.to_owned())
        }
        fn visit_string<E: serde::de::Error>(self, value: String) -> Result<String, E> {
            if value.len() > MAX_NAME_BYTES {
                return Err(E::custom("schema field name exceeds 256 bytes"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_string(Name).map(Cow::Owned)
}

/* === Tests === */
