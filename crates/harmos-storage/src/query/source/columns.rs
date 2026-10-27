/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Decode Validated Payload Events Into Arrow Columns

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::{ArrayRef, ListArray, StructArray, builder::*};
use arrow_buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow_schema::{DataType, FieldRef, Fields};

// Local Imports
use super::invalid;
use crate::Result;
use crate::schema::{Event, PathElement, Scalar};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub(super) enum Column {
    Scalar(Box<dyn ArrayBuilder>),
    Struct { fields: Fields, children: Vec<Column>, valid: Vec<bool> },
    List { field: FieldRef, child: Box<Column>, offsets: Vec<i32>, valid: Vec<bool> },
}

/* === Traits === */

/* === Implementations === */

impl Column {
    pub fn new(kind: &DataType, capacity: usize) -> Self {
        match kind {
            DataType::Struct(fields) => Self::Struct {
                fields: fields.clone(),
                children: fields.iter().map(|field| Self::new(field.data_type(), capacity)).collect(),
                valid: Vec::with_capacity(capacity),
            },
            DataType::List(field) => Self::List {
                field: field.clone(),
                child: Box::new(Self::new(field.data_type(), capacity)),
                offsets: vec![0],
                valid: Vec::with_capacity(capacity),
            },
            kind => Self::Scalar(make_builder(kind, capacity)),
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Scalar(builder) => builder.len(),
            Self::Struct { valid, .. } | Self::List { valid, .. } => valid.len(),
        }
    }

    fn at(&mut self, path: &[PathElement<'_>]) -> Result<&mut Self> {
        let Some((first, rest)) = path.split_first() else { return Ok(self) };
        match (self, first) {
            (Self::Struct { fields, children, .. }, PathElement::Field { name, .. }) => {
                let index = fields.iter().position(|field| field.name() == name).ok_or_else(|| invalid("unknown payload field"))?;
                children[index].at(rest)
            }
            (Self::List { child, .. }, PathElement::Index(_)) => child.at(rest),
            _ => Err(invalid("payload path differs from schema")),
        }
    }

    pub fn event(&mut self, path: &[PathElement<'_>], event: Event<'_>) -> Result<()> {
        // Container Starts Carry No Values; Ends Commit Their Child Columns.
        if matches!(event, Event::BeginMessage | Event::BeginRepeated { .. } | Event::Optional { present: true }) {
            return Ok(());
        }
        if let Event::Variant { index } = event {
            let parent = &path[..path.len().saturating_sub(1)];
            let Self::Struct { children, .. } = self.at(parent)? else { return Err(invalid("invalid enum parent")) };
            return children[0].scalar(Some(Scalar::U32(index)));
        }
        let column = self.at(path)?;
        match event {
            Event::Scalar(value) => column.scalar(Some(value)),
            Event::Optional { present: false } => column.null(),
            Event::EndMessage => column.end_struct(true),
            Event::EndRepeated => column.end_list(true),
            _ => unreachable!("handled container starts and discriminants"),
        }
    }

    pub(super) fn null(&mut self) -> Result<()> {
        match self {
            Self::Scalar(_) => self.scalar(None),
            Self::Struct { .. } => self.end_struct(false),
            Self::List { .. } => self.end_list(false),
        }
    }

    fn end_struct(&mut self, present: bool) -> Result<()> {
        let Self::Struct { children, valid, .. } = self else { return Err(invalid("expected message column")) };
        for child in children {
            if child.len() == valid.len() {
                child.null()?;
            }
            if child.len() != valid.len() + 1 {
                return Err(invalid("payload child length differs from parent"));
            }
        }
        valid.push(present);
        Ok(())
    }

    fn end_list(&mut self, present: bool) -> Result<()> {
        let Self::List { child, offsets, valid, .. } = self else { return Err(invalid("expected repeated column")) };
        let offset = i32::try_from(child.len()).map_err(|_| invalid("list exceeds Arrow offset range"))?;
        offsets.push(offset);
        valid.push(present);
        Ok(())
    }

    fn scalar(&mut self, value: Option<Scalar<'_>>) -> Result<()> {
        let Self::Scalar(builder) = self else { return Err(invalid("expected scalar column")) };
        macro_rules! append {
            ($ty:ty, $variant:ident) => {
                if let Some(builder) = builder.as_any_mut().downcast_mut::<$ty>() {
                    match value {
                        Some(Scalar::$variant(value)) => builder.append_value(value),
                        None => builder.append_null(),
                        _ => return Err(invalid("payload value differs from declared schema")),
                    }
                    return Ok(());
                }
            };
        }
        append!(Float64Builder, F64);
        append!(UInt64Builder, U64);
        append!(Int64Builder, I64);
        append!(Float32Builder, F32);
        append!(UInt32Builder, U32);
        append!(Int32Builder, I32);
        append!(BooleanBuilder, Bool);
        append!(StringBuilder, String);
        append!(BinaryBuilder, Bytes);
        Err(invalid("unsupported payload type"))
    }

    pub fn finish(self) -> Result<ArrayRef> {
        match self {
            Self::Scalar(mut builder) => Ok(builder.finish()),
            Self::Struct { fields, children, valid } => {
                if fields.is_empty() {
                    return Ok(Arc::new(StructArray::new_empty_fields(valid.len(), Some(NullBuffer::from(valid)))));
                }
                let children = children.into_iter().map(Self::finish).collect::<Result<Vec<_>>>()?;
                let array = StructArray::try_new(fields, children, Some(NullBuffer::from(valid)))
                    .map_err(|_| invalid("nested Arrow schema mismatch"))?;
                Ok(Arc::new(array))
            }
            Self::List { field, child, offsets, valid } => {
                let offsets = OffsetBuffer::new(ScalarBuffer::from(offsets));
                let child = child.finish()?;
                let array = ListArray::try_new(field, offsets, child, Some(NullBuffer::from(valid)))
                    .map_err(|_| invalid("list Arrow schema mismatch"))?;
                Ok(Arc::new(array))
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
