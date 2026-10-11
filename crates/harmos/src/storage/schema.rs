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
use std::fmt;
// Third Party Imports
use serde::de::{DeserializeSeed, Error as _, SeqAccess, Visitor};
use serde::ser::{Error as _, SerializeSeq, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports
use super::manager::encoded_size;
use super::{SchemaKey, StorageCursor, StorageSchema};
use crate::schema::{Cardinality, Field, Kind, MAX_DEPTH, MAX_FIELDS, MAX_NAME_BYTES};
use crate::{Error, Result};

/* === Constants === */
const MAX_DEFINITIONS: usize = MAX_FIELDS + 1;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Plan<'a> {
    definitions: Vec<&'a [Field]>,
    root: usize,
}

struct DefinitionsRef<'a>(&'a [&'a [Field]]);

struct DefinitionRef<'a> {
    fields: &'a [Field],
    definitions: &'a [&'a [Field]],
}

struct FieldRef<'a> {
    field: &'a Field,
    definitions: &'a [&'a [Field]],
}

struct KindRef<'a> {
    kind: &'a Kind,
    definitions: &'a [&'a [Field]],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    position: StorageCursor,
    key: SchemaKey,
    #[serde(default, deserialize_with = "present")]
    fields: Option<Vec<Field>>,
    #[serde(default, deserialize_with = "present")]
    encoding: Option<SchemaEncoding>,
    #[serde(default, deserialize_with = "present")]
    root: Option<usize>,
    #[serde(default, deserialize_with = "present")]
    definitions: Option<Definitions>,
}

struct Definitions(Vec<Vec<GraphField>>);

struct FieldsSeed<'a>(&'a mut usize);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphField {
    #[serde(deserialize_with = "name")]
    name: String,
    tag: u32,
    kind: GraphKind,
    cardinality: Cardinality,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphKind {
    #[serde(rename = "type")]
    kind: GraphType,
    #[serde(default, deserialize_with = "present")]
    definition: Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct Expanded {
    fields: usize,
    names: usize,
    depth: usize,
}

/* === Enums === */
enum GraphType {
    Scalar(Kind),
    Message,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaEncoding {
    #[default]
    Inline,
    GraphV1,
}

/* === Traits === */

/* === Implementations === */
impl StorageSchema {
    pub(super) fn encoded_bytes(&self) -> Result<usize> {
        if self.encoding() == SchemaEncoding::Inline {
            return encoded_size(self);
        }
        let size_error = || Error::InvalidInput {
            field: "storage payload",
            reason: "cannot measure encoded size",
        };
        crate::schema::validation_memory_bytes(&self.fields).map_err(|_| size_error())?;
        let mut plan = Plan {
            definitions: Vec::with_capacity(occurrences(&self.fields)),
            root: 0,
        };
        plan.root = plan.intern(&self.fields);

        // Count The Current Graph Without Emitting Every JSON Token.
        // Serde Still Owns Escaping And Scalar/Enum Representations.
        let scalars = [
            encoded_size(&Kind::Bool)?,
            encoded_size(&Kind::U32)?,
            encoded_size(&Kind::U64)?,
            encoded_size(&Kind::I32)?,
            encoded_size(&Kind::I64)?,
            encoded_size(&Kind::F32)?,
            encoded_size(&Kind::F64)?,
            encoded_size(&Kind::String)?,
            encoded_size(&Kind::Bytes)?,
        ];
        let cardinalities = [
            encoded_size(&Cardinality::Required)?,
            encoded_size(&Cardinality::Optional)?,
            encoded_size(&Cardinality::Repeated)?,
            encoded_size(&Cardinality::Variant)?,
        ];
        let mut bytes = 0usize;
        for size in [
            r#"{"position":,"key":,"encoding":,"root":,"definitions":}"#.len(),
            encoded_size(&self.position)?,
            encoded_size(&self.key)?,
            encoded_size(&self.encoding())?,
            encoded_size(&plan.root)?,
            2 + plan.definitions.len().saturating_sub(1),
        ] {
            bytes = bytes.checked_add(size).ok_or_else(size_error)?;
        }
        for fields in &plan.definitions {
            bytes = bytes
                .checked_add(2 + fields.len().saturating_sub(1))
                .ok_or_else(size_error)?;
            for field in *fields {
                let kind = match &field.kind {
                    Kind::Bool => scalars[0],
                    Kind::U32 => scalars[1],
                    Kind::U64 => scalars[2],
                    Kind::I32 => scalars[3],
                    Kind::I64 => scalars[4],
                    Kind::F32 => scalars[5],
                    Kind::F64 => scalars[6],
                    Kind::String => scalars[7],
                    Kind::Bytes => scalars[8],
                    Kind::Message { fields } => {
                        let definition = plan
                            .definitions
                            .iter()
                            .position(|existing| {
                                std::ptr::eq(*existing, fields.as_ref())
                                    || *existing == fields.as_ref()
                            })
                            .ok_or_else(size_error)?;
                        r#"{"type":"message","definition":}"#.len() + encoded_size(&definition)?
                    }
                };
                let cardinality = match field.cardinality {
                    Cardinality::Required => cardinalities[0],
                    Cardinality::Optional => cardinalities[1],
                    Cardinality::Repeated => cardinalities[2],
                    Cardinality::Variant => cardinalities[3],
                };
                for size in [
                    r#"{"name":,"tag":,"kind":,"cardinality":}"#.len(),
                    encoded_size(&field.name)?,
                    encoded_size(&field.tag)?,
                    kind,
                    cardinality,
                ] {
                    bytes = bytes.checked_add(size).ok_or_else(size_error)?;
                }
            }
        }
        Ok(bytes)
    }
}

impl SchemaEncoding {
    pub fn decoding_overhead(self) -> usize {
        if self == Self::Inline {
            return 0;
        }
        2 * MAX_DEFINITIONS * std::mem::size_of::<Vec<GraphField>>()
            + MAX_DEFINITIONS * std::mem::size_of::<&[Field]>()
            + MAX_DEFINITIONS * (std::mem::size_of::<Expanded>() + 1)
            + MAX_FIELDS * (std::mem::size_of::<Field>() + MAX_NAME_BYTES)
            + MAX_FIELDS * (std::mem::size_of::<&str>() + std::mem::size_of::<u32>())
            + MAX_DEFINITIONS * (std::mem::size_of::<Vec<&str>>() + std::mem::size_of::<Vec<u32>>())
            + MAX_NAME_BYTES
            + std::mem::size_of::<crate::schema::Error>()
    }

    pub fn encoding_memory_bytes(self, fields: &[Field]) -> Result<usize> {
        let validation = crate::schema::validation_memory_bytes(fields).map_err(|_| invalid())?;
        Ok(validation.max(self.planning_bytes(fields)?))
    }

    pub(super) fn planning_bytes(self, fields: &[Field]) -> Result<usize> {
        if self == Self::Inline {
            return Ok(0);
        }
        crate::schema::validation_memory_bytes(fields).map_err(|_| invalid())?;
        occurrences(fields)
            .checked_mul(std::mem::size_of::<&[Field]>())
            .ok_or_else(invalid)
    }
}

impl Serialize for StorageSchema {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        if self.encoding() == SchemaEncoding::Inline {
            let mut output = serializer.serialize_struct("StorageSchema", 3)?;
            output.serialize_field("position", &self.position)?;
            output.serialize_field("key", &self.key)?;
            output.serialize_field("fields", &self.fields)?;
            return output.end();
        }
        crate::schema::validation_memory_bytes(&self.fields).map_err(S::Error::custom)?;
        let mut plan = Plan {
            definitions: Vec::with_capacity(occurrences(&self.fields)),
            root: 0,
        };
        plan.root = plan.intern(&self.fields);
        let mut output = serializer.serialize_struct("StorageSchema", 5)?;
        output.serialize_field("position", &self.position)?;
        output.serialize_field("key", &self.key)?;
        output.serialize_field("encoding", &SchemaEncoding::GraphV1)?;
        output.serialize_field("root", &plan.root)?;
        output.serialize_field("definitions", &DefinitionsRef(&plan.definitions))?;
        output.end()
    }
}

impl<'de> Deserialize<'de> for StorageSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let input = Input::deserialize(deserializer)?;
        let (fields, encoding) = match (input.encoding, input.fields, input.root, input.definitions)
        {
            (None, Some(fields), None, None) => (fields, SchemaEncoding::Inline),
            (Some(SchemaEncoding::GraphV1), None, Some(root), Some(definitions)) => (
                definitions.expand(root).map_err(D::Error::custom)?,
                SchemaEncoding::GraphV1,
            ),
            _ => return Err(D::Error::custom("invalid stored schema encoding envelope")),
        };
        crate::schema::validate(&fields).map_err(D::Error::custom)?;
        Ok(Self::new(input.position, input.key, fields, encoding))
    }
}

impl<'a> Plan<'a> {
    fn intern(&mut self, fields: &'a [Field]) -> usize {
        // An Existing Definition Already Owns Its Complete Postorder Subtree.
        if let Some(index) = self
            .definitions
            .iter()
            .position(|existing| std::ptr::eq(*existing, fields) || *existing == fields)
        {
            return index;
        }
        for field in fields {
            if let Kind::Message { fields } = &field.kind {
                self.intern(fields);
            }
        }
        let index = self.definitions.len();
        self.definitions.push(fields);
        index
    }
}

impl Serialize for DefinitionsRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_seq(Some(self.0.len()))?;
        for fields in self.0 {
            output.serialize_element(&DefinitionRef {
                fields,
                definitions: self.0,
            })?;
        }
        output.end()
    }
}

impl Serialize for DefinitionRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_seq(Some(self.fields.len()))?;
        for field in self.fields {
            output.serialize_element(&FieldRef {
                field,
                definitions: self.definitions,
            })?;
        }
        output.end()
    }
}

impl Serialize for FieldRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("Field", 4)?;
        output.serialize_field("name", &self.field.name)?;
        output.serialize_field("tag", &self.field.tag)?;
        output.serialize_field(
            "kind",
            &KindRef {
                kind: &self.field.kind,
                definitions: self.definitions,
            },
        )?;
        output.serialize_field("cardinality", &self.field.cardinality)?;
        output.end()
    }
}

impl Serialize for KindRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let Kind::Message { fields } = self.kind else {
            return self.kind.serialize(serializer);
        };
        let definition = self
            .definitions
            .iter()
            .position(|existing| {
                std::ptr::eq(*existing, fields.as_ref()) || *existing == fields.as_ref()
            })
            .ok_or_else(|| S::Error::custom("missing local schema definition"))?;
        let mut output = serializer.serialize_struct("Kind", 2)?;
        output.serialize_field("type", "message")?;
        output.serialize_field("definition", &definition)?;
        output.end()
    }
}

impl<'de> Deserialize<'de> for Definitions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct DefinitionsVisitor;
        impl<'de> Visitor<'de> for DefinitionsVisitor {
            type Value = Definitions;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bounded local schema definitions")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut definitions = Vec::new();
                let mut remaining = MAX_FIELDS;
                while let Some(fields) = sequence.next_element_seed(FieldsSeed(&mut remaining))? {
                    if definitions.len() == MAX_DEFINITIONS {
                        return Err(A::Error::custom("too many local schema definitions"));
                    }
                    definitions.push(fields);
                }
                Ok(Definitions(definitions))
            }
        }
        deserializer.deserialize_seq(DefinitionsVisitor)
    }
}

impl<'de> DeserializeSeed<'de> for FieldsSeed<'_> {
    type Value = Vec<GraphField>;
    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        struct FieldsVisitor<'a>(&'a mut usize);
        impl<'de> Visitor<'de> for FieldsVisitor<'_> {
            type Value = Vec<GraphField>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bounded local schema fields")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut fields = Vec::new();
                while let Some(field) = sequence.next_element::<GraphField>()? {
                    if *self.0 == 0 {
                        return Err(A::Error::custom("too many local schema fields"));
                    }
                    *self.0 -= 1;
                    if fields.is_empty() {
                        fields = Vec::with_capacity(1);
                    }
                    fields.push(field);
                }
                Ok(fields)
            }
        }
        deserializer.deserialize_seq(FieldsVisitor(self.0))
    }
}

impl Definitions {
    fn expand(self, root: usize) -> Result<Vec<Field>> {
        if self.0.is_empty() || root != self.0.len() - 1 {
            return Err(invalid());
        }
        // Validate References And Expansion Before Allocating The Ordered Tree.
        let mut expanded: Vec<Expanded> = Vec::with_capacity(self.0.len());
        for (index, fields) in self.0.iter().enumerate() {
            let mut total = Expanded {
                fields: fields.len(),
                names: 0,
                depth: 0,
            };
            for field in fields {
                total.names = total
                    .names
                    .checked_add(field.name.len())
                    .ok_or_else(invalid)?;
                match (&field.kind.kind, field.kind.definition) {
                    (GraphType::Message, Some(child)) if child < index => {
                        total.fields = total
                            .fields
                            .checked_add(expanded[child].fields)
                            .ok_or_else(invalid)?;
                        total.names = total
                            .names
                            .checked_add(expanded[child].names)
                            .ok_or_else(invalid)?;
                        total.depth = total.depth.max(expanded[child].depth + 1);
                    }
                    (GraphType::Message, _) | (_, Some(_)) => return Err(invalid()),
                    (GraphType::Scalar(_), None) => {}
                }
            }
            if total.fields > MAX_FIELDS || total.depth > MAX_DEPTH {
                return Err(invalid());
            }
            expanded.push(total);
        }
        let mut reachable = vec![false; self.0.len()];
        reachable[root] = true;
        for index in (0..=root).rev() {
            if reachable[index] {
                for field in &self.0[index] {
                    if let Some(child) = field.kind.definition {
                        reachable[child] = true;
                    }
                }
            }
        }
        if reachable.iter().any(|reachable| !reachable) {
            return Err(invalid());
        }
        let fields = self.fields(root)?;
        // Canonical Graph Identity Includes Structural Interning And Postorder.
        let mut plan = Plan {
            definitions: Vec::with_capacity(occurrences(&fields)),
            root: 0,
        };
        plan.root = plan.intern(&fields);
        if plan.root != root || plan.definitions.len() != self.0.len() {
            return Err(invalid());
        }
        for (fields, original) in plan.definitions.iter().zip(&self.0) {
            if fields.len() != original.len() {
                return Err(invalid());
            }
            for (field, raw) in fields.iter().zip(original) {
                if field.name != raw.name
                    || field.tag != raw.tag
                    || field.cardinality != raw.cardinality
                {
                    return Err(invalid());
                }
                if let (Kind::Message { fields }, Some(child)) = (&field.kind, raw.kind.definition)
                {
                    if fields.as_ref() != plan.definitions[child] {
                        return Err(invalid());
                    }
                } else if field.kind != raw.kind.scalar()? {
                    return Err(invalid());
                }
            }
        }
        Ok(fields)
    }

    fn fields(&self, index: usize) -> Result<Vec<Field>> {
        let mut fields = Vec::with_capacity(self.0[index].len());
        for field in &self.0[index] {
            let kind = if let Some(child) = field.kind.definition {
                Kind::Message {
                    fields: Cow::Owned(self.fields(child)?),
                }
            } else {
                field.kind.scalar()?
            };
            fields.push(Field {
                name: Cow::Owned(field.name.clone()),
                tag: field.tag,
                kind,
                cardinality: field.cardinality,
            });
        }
        Ok(fields)
    }
}

impl GraphKind {
    fn scalar(&self) -> Result<Kind> {
        match &self.kind {
            GraphType::Scalar(kind) => Ok(kind.clone()),
            GraphType::Message => Err(invalid()),
        }
    }
}

impl<'de> Deserialize<'de> for GraphType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Type;
        impl Visitor<'_> for Type {
            type Value = GraphType;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a schema scalar or local message reference")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<GraphType, E> {
                if value == "message" {
                    return Ok(GraphType::Message);
                }
                let value = serde::de::value::StrDeserializer::<E>::new(value);
                let scalar = Kind::deserialize(serde::de::value::MapDeserializer::new(
                    std::iter::once(("type", value)),
                ))
                .map_err(|_| E::custom("invalid schema scalar"))?;
                Ok(GraphType::Scalar(scalar))
            }
        }
        deserializer.deserialize_str(Type)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn occurrences(fields: &[Field]) -> usize {
    1 + fields
        .iter()
        .filter_map(|field| match &field.kind {
            Kind::Message { fields } => Some(occurrences(fields)),
            _ => None,
        })
        .sum::<usize>()
}

fn name<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<String, D::Error> {
    struct Name;
    impl Visitor<'_> for Name {
        type Value = String;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded schema name")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<String, E> {
            if value.len() > MAX_NAME_BYTES {
                return Err(E::custom("schema name exceeds its bound"));
            }
            Ok(value.to_owned())
        }
        fn visit_string<E: serde::de::Error>(
            self,
            value: String,
        ) -> std::result::Result<String, E> {
            if value.len() > MAX_NAME_BYTES {
                return Err(E::custom("schema name exceeds its bound"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_string(Name)
}

fn invalid() -> Error {
    Error::InvalidInput {
        field: "storage schema",
        reason: "invalid or excessive local schema graph",
    }
}

/* === Tests === */
