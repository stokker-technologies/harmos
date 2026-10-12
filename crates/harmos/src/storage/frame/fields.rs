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
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::super::reduce::ScalarValue;
use super::{ScalarField, ScalarPath, heap, invalid};
use crate::{MemorySize, Result, schema};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ScalarProjection {
    validator: schema::Validator,
    fields: Vec<ScalarField>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl ScalarProjection {
    pub fn new(
        schema: &[schema::Field],
        fields: Vec<ScalarField>,
        memory_bytes: NonZeroUsize,
    ) -> Result<Self> {
        if fields.is_empty() || fields.len() > 64 {
            return Err(invalid("projection requires 1..64 fields"));
        }
        for field in &fields {
            field.check_schema(schema)?;
        }
        let construction = schema::Validator::construction_memory_bytes(schema)
            .map_err(|_| invalid("invalid projection schema"))?;
        if construction
            .saturating_add(fields.memory_size())
            .saturating_add(std::mem::size_of::<Self>())
            > memory_bytes.get()
        {
            return Err(invalid("projection construction exceeds memory"));
        }
        let validator =
            schema::Validator::new(schema).map_err(|_| invalid("invalid projection schema"))?;
        Ok(Self { validator, fields })
    }

    pub fn project(
        &self,
        bytes: &[u8],
        metadata: impl Fn(&ScalarPath, NonZeroUsize) -> Result<ScalarValue>,
        memory_bytes: NonZeroUsize,
    ) -> Result<Vec<ScalarValue>> {
        let mut used = std::mem::size_of::<Vec<ScalarValue>>().saturating_add(
            self.fields
                .len()
                .saturating_mul(std::mem::size_of::<ScalarValue>()),
        );
        if used > memory_bytes.get() {
            return Err(invalid("projection output exceeds memory"));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.fields.len())
            .map_err(|_| invalid("projection allocation failed"))?;
        if values.capacity() != self.fields.len() {
            return Err(invalid("projection capacity exceeds reservation"));
        }
        for field in &self.fields {
            let value = if matches!(field.path, ScalarPath::Payload(_)) {
                ScalarValue::Missing
            } else {
                metadata(
                    &field.path,
                    NonZeroUsize::new(memory_bytes.get().saturating_sub(used))
                        .ok_or(invalid("metadata output exceeds memory"))?,
                )?
            };
            value.check()?;
            if value.kind().is_some_and(|kind| kind != field.kind) {
                return Err(invalid("metadata type differs"));
            }
            used = used.saturating_add(heap(&value));
            if used > memory_bytes.get() {
                return Err(invalid("metadata output exceeds memory"));
            }
            values.push(value);
        }
        self.validator.visit(bytes, |path, event| {
            for (index, field) in self.fields.iter().enumerate() {
                let ScalarPath::Payload(names) = &field.path else { continue; };
                let prefix = path.len() <= names.len() && path.iter().zip(names).all(|(element, name)| matches!(element, schema::PathElement::Field { name: field, .. } if *field == name.as_str()));
                if !prefix { continue; }
                if matches!(event, schema::Event::Optional { present: false }) {
                    values[index] = ScalarValue::Null;
                    continue;
                }
                if path.len() != names.len() { continue; }
                let schema::Event::Scalar(value) = event else { continue; };
                let value = match value {
                    schema::Scalar::Bool(v) => ScalarValue::Bool(v),
                    schema::Scalar::I32(v) => ScalarValue::Signed(i64::from(v)),
                    schema::Scalar::I64(v) => ScalarValue::Signed(v),
                    schema::Scalar::U32(v) => ScalarValue::Unsigned(u64::from(v)),
                    schema::Scalar::U64(v) => ScalarValue::Unsigned(v),
                    schema::Scalar::F32(v) => ScalarValue::Float(f64::from(v)),
                    schema::Scalar::F64(v) => ScalarValue::Float(v),
                    schema::Scalar::String(v) => {
                        used = used.saturating_add(v.len());
                        if used > memory_bytes.get() { return Err(invalid("text projection exceeds memory")); }
                        let mut text = String::new();
                        text.try_reserve_exact(v.len()).map_err(|_| invalid("text allocation failed"))?;
                        if text.capacity() > v.len() { return Err(invalid("text capacity exceeds reservation")); }
                        text.push_str(v);
                        ScalarValue::Text(text)
                    }
                    _ => return Err(invalid("unsupported projected value")),
                };
                value.check()?;
                values[index] = value;
            }
            Ok(())
        }).map_err(|error| match error { schema::VisitError::Consumer(error) => error, schema::VisitError::Payload(_) => invalid("invalid persisted postcard payload") })?;
        Ok(values)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
