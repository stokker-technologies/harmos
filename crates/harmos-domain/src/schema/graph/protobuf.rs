/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Convert Generated Storage Schema Descriptions to Native Graphs.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Field, Kind, Schema};
use crate::schema::{
    Cardinality,
    wire::{Form, Omission, Representation},
};
use crate::{Error, Result};
use harmos_protocol::storage as wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<Cardinality> for wire::SchemaCardinality {
    fn from(value: Cardinality) -> Self {
        match value {
            Cardinality::Required => Self::Required,
            Cardinality::Optional => Self::Optional,
            Cardinality::Repeated => Self::Repeated,
            Cardinality::Variant => Self::Variant,
        }
    }
}
impl TryFrom<i32> for Cardinality {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::SchemaCardinality::try_from(value) {
            Ok(wire::SchemaCardinality::Required) => Ok(Self::Required),
            Ok(wire::SchemaCardinality::Optional) => Ok(Self::Optional),
            Ok(wire::SchemaCardinality::Repeated) => Ok(Self::Repeated),
            Ok(wire::SchemaCardinality::Variant) => Ok(Self::Variant),
            Err(_) => Err(invalid("unknown SchemaCardinality")),
        }
    }
}
impl From<Form> for wire::SchemaForm {
    fn from(value: Form) -> Self {
        match value {
            Form::Object => Self::Object,
            Form::Unit => Self::Unit,
            Form::Value => Self::Value,
            Form::Tuple => Self::Tuple,
            Form::External => Self::External,
            Form::Internal => Self::Internal,
            Form::Adjacent => Self::Adjacent,
            Form::Untagged => Self::Untagged,
        }
    }
}
impl TryFrom<i32> for Form {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::SchemaForm::try_from(value) {
            Ok(wire::SchemaForm::Object) => Ok(Self::Object),
            Ok(wire::SchemaForm::Unit) => Ok(Self::Unit),
            Ok(wire::SchemaForm::Value) => Ok(Self::Value),
            Ok(wire::SchemaForm::Tuple) => Ok(Self::Tuple),
            Ok(wire::SchemaForm::External) => Ok(Self::External),
            Ok(wire::SchemaForm::Internal) => Ok(Self::Internal),
            Ok(wire::SchemaForm::Adjacent) => Ok(Self::Adjacent),
            Ok(wire::SchemaForm::Untagged) => Ok(Self::Untagged),
            Err(_) => Err(invalid("unknown SchemaForm")),
        }
    }
}
impl From<Omission> for wire::SchemaOmission {
    fn from(value: Omission) -> Self {
        match value {
            Omission::None => Self::OmitNone,
            Omission::Empty => Self::OmitEmpty,
        }
    }
}
impl TryFrom<i32> for Omission {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::SchemaOmission::try_from(value) {
            Ok(wire::SchemaOmission::OmitNone) => Ok(Self::None),
            Ok(wire::SchemaOmission::OmitEmpty) => Ok(Self::Empty),
            Err(_) => Err(invalid("unknown SchemaOmission")),
        }
    }
}
impl From<Field> for wire::SchemaField {
    fn from(value: Field) -> Self {
        let (kind, definition) = match value.kind {
            Kind::Bool => (wire::SchemaKind::Bool, None),
            Kind::U32 => (wire::SchemaKind::Uint32, None),
            Kind::U64 => (wire::SchemaKind::Uint64, None),
            Kind::I32 => (wire::SchemaKind::Int32, None),
            Kind::I64 => (wire::SchemaKind::Int64, None),
            Kind::F32 => (wire::SchemaKind::Float32, None),
            Kind::F64 => (wire::SchemaKind::Float64, None),
            Kind::String => (wire::SchemaKind::String, None),
            Kind::Bytes => (wire::SchemaKind::Bytes, None),
            Kind::Message { definition } => (wire::SchemaKind::Message, Some(definition as u64)),
        };
        Self {
            name: value.name.into_owned(),
            tag: value.tag,
            kind: kind as i32,
            cardinality: wire::SchemaCardinality::from(value.cardinality) as i32,
            definition,
        }
    }
}
impl TryFrom<wire::SchemaField> for Field {
    type Error = Error;
    fn try_from(value: wire::SchemaField) -> Result<Self> {
        let kind = wire::SchemaKind::try_from(value.kind).map_err(|_| invalid("unknown schema field kind"))?;
        if kind != wire::SchemaKind::Message && value.definition.is_some() {
            return Err(invalid("scalar field cannot reference a definition"));
        }
        let kind = match kind {
            wire::SchemaKind::Bool => Kind::Bool,
            wire::SchemaKind::Uint32 => Kind::U32,
            wire::SchemaKind::Uint64 => Kind::U64,
            wire::SchemaKind::Int32 => Kind::I32,
            wire::SchemaKind::Int64 => Kind::I64,
            wire::SchemaKind::Float32 => Kind::F32,
            wire::SchemaKind::Float64 => Kind::F64,
            wire::SchemaKind::String => Kind::String,
            wire::SchemaKind::Bytes => Kind::Bytes,
            wire::SchemaKind::Message => Kind::Message {
                definition: value
                    .definition
                    .ok_or_else(|| invalid("message definition is required"))?
                    .try_into()
                    .map_err(|_| invalid("definition index exceeds platform range"))?,
            },
        };
        Ok(Self {
            name: value.name.into(),
            tag: value.tag,
            cardinality: value.cardinality.try_into()?,
            kind,
        })
    }
}
impl From<Schema> for wire::SchemaGraph {
    fn from(value: Schema) -> Self {
        Self {
            root: value.root as u64,
            definitions: value
                .definitions
                .into_iter()
                .map(|fields| wire::SchemaDefinition {
                    fields: fields.into_iter().map(Into::into).collect(),
                })
                .collect(),
            representations: value
                .representations
                .into_iter()
                .map(|(definition, value)| wire::SchemaRepresentation {
                    definition: definition as u64,
                    form: wire::SchemaForm::from(value.form) as i32,
                    tag: value.tag,
                    content: value.content,
                    omissions: value.omissions.into_iter().map(|(key, value)| (key, wire::SchemaOmission::from(value) as i32)).collect(),
                    maps: value.maps,
                })
                .collect(),
        }
    }
}
impl TryFrom<wire::SchemaGraph> for Schema {
    type Error = Error;
    fn try_from(value: wire::SchemaGraph) -> Result<Self> {
        let mut representations = std::collections::BTreeMap::new();
        for representation in value.representations {
            let index = representation.definition.try_into().map_err(|_| invalid("representation index exceeds platform range"))?;
            let native = Representation {
                form: representation.form.try_into()?,
                tag: representation.tag,
                content: representation.content,
                omissions: representation.omissions.into_iter().map(|(key, value)| Ok((key, value.try_into()?))).collect::<Result<_>>()?,
                maps: representation.maps,
            };
            if representations.insert(index, native).is_some() {
                return Err(invalid("duplicate representation index"));
            }
        }
        Ok(Self {
            root: value.root.try_into().map_err(|_| invalid("root index exceeds platform range"))?,
            definitions: value
                .definitions
                .into_iter()
                .map(|definition| definition.fields.into_iter().map(TryInto::try_into).collect())
                .collect::<Result<_>>()?,
            representations,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage schema", reason }
}

/* === Tests === */
