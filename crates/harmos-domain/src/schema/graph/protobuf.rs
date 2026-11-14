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

use harmos_protocol::host::SchemaCardinality as WireSchemaCardinality;
use harmos_protocol::host::SchemaDefinition;
use harmos_protocol::host::SchemaField;
use harmos_protocol::host::SchemaForm as WireSchemaForm;
use harmos_protocol::host::SchemaGraph;
use harmos_protocol::host::SchemaKind;
use harmos_protocol::host::SchemaOmission as WireSchemaOmission;
use harmos_protocol::host::SchemaRepresentation;

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Field, Kind, Schema};
use crate::schema::{
    Cardinality,
    wire::{Form, Omission, Representation},
};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<Cardinality> for WireSchemaCardinality {
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
        match WireSchemaCardinality::try_from(value) {
            Ok(WireSchemaCardinality::Required) => Ok(Self::Required),
            Ok(WireSchemaCardinality::Optional) => Ok(Self::Optional),
            Ok(WireSchemaCardinality::Repeated) => Ok(Self::Repeated),
            Ok(WireSchemaCardinality::Variant) => Ok(Self::Variant),
            Err(_) => Err(invalid("unknown SchemaCardinality")),
        }
    }
}
impl From<Form> for WireSchemaForm {
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
        match WireSchemaForm::try_from(value) {
            Ok(WireSchemaForm::Object) => Ok(Self::Object),
            Ok(WireSchemaForm::Unit) => Ok(Self::Unit),
            Ok(WireSchemaForm::Value) => Ok(Self::Value),
            Ok(WireSchemaForm::Tuple) => Ok(Self::Tuple),
            Ok(WireSchemaForm::External) => Ok(Self::External),
            Ok(WireSchemaForm::Internal) => Ok(Self::Internal),
            Ok(WireSchemaForm::Adjacent) => Ok(Self::Adjacent),
            Ok(WireSchemaForm::Untagged) => Ok(Self::Untagged),
            Err(_) => Err(invalid("unknown SchemaForm")),
        }
    }
}
impl From<Omission> for WireSchemaOmission {
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
        match WireSchemaOmission::try_from(value) {
            Ok(WireSchemaOmission::OmitNone) => Ok(Self::None),
            Ok(WireSchemaOmission::OmitEmpty) => Ok(Self::Empty),
            Err(_) => Err(invalid("unknown SchemaOmission")),
        }
    }
}
impl From<Field> for SchemaField {
    fn from(value: Field) -> Self {
        let (kind, definition) = match value.kind {
            Kind::Bool => (SchemaKind::Bool, None),
            Kind::U32 => (SchemaKind::Uint32, None),
            Kind::U64 => (SchemaKind::Uint64, None),
            Kind::I32 => (SchemaKind::Int32, None),
            Kind::I64 => (SchemaKind::Int64, None),
            Kind::F32 => (SchemaKind::Float32, None),
            Kind::F64 => (SchemaKind::Float64, None),
            Kind::String => (SchemaKind::String, None),
            Kind::Bytes => (SchemaKind::Bytes, None),
            Kind::Message { definition } => (SchemaKind::Message, Some(definition as u64)),
        };
        Self {
            name: value.name.into_owned(),
            tag: value.tag,
            kind: kind as i32,
            cardinality: WireSchemaCardinality::from(value.cardinality) as i32,
            definition,
        }
    }
}
impl TryFrom<SchemaField> for Field {
    type Error = Error;
    fn try_from(value: SchemaField) -> Result<Self> {
        let kind = SchemaKind::try_from(value.kind).map_err(|_| invalid("unknown schema field kind"))?;
        if kind != SchemaKind::Message && value.definition.is_some() {
            return Err(invalid("scalar field cannot reference a definition"));
        }
        let kind = match kind {
            SchemaKind::Bool => Kind::Bool,
            SchemaKind::Uint32 => Kind::U32,
            SchemaKind::Uint64 => Kind::U64,
            SchemaKind::Int32 => Kind::I32,
            SchemaKind::Int64 => Kind::I64,
            SchemaKind::Float32 => Kind::F32,
            SchemaKind::Float64 => Kind::F64,
            SchemaKind::String => Kind::String,
            SchemaKind::Bytes => Kind::Bytes,
            SchemaKind::Message => Kind::Message {
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
impl From<Schema> for SchemaGraph {
    fn from(value: Schema) -> Self {
        Self {
            root: value.root as u64,
            definitions: value
                .definitions
                .into_iter()
                .map(|fields| SchemaDefinition {
                    fields: fields.into_iter().map(Into::into).collect(),
                })
                .collect(),
            representations: value
                .representations
                .into_iter()
                .map(|(definition, value)| SchemaRepresentation {
                    definition: definition as u64,
                    form: WireSchemaForm::from(value.form) as i32,
                    tag: value.tag,
                    content: value.content,
                    omissions: value.omissions.into_iter().map(|(key, value)| (key, WireSchemaOmission::from(value) as i32)).collect(),
                    maps: value.maps,
                })
                .collect(),
        }
    }
}
impl TryFrom<SchemaGraph> for Schema {
    type Error = Error;
    fn try_from(value: SchemaGraph) -> Result<Self> {
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
