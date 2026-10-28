/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

pub use harmos_domain::model::storage::query::transform::project::{Project, Projection};

use crate::storage::query::transform::FieldPathExecution;

use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::Schema;

use super::{FieldPath, invalid};
use crate::Result;

pub trait ProjectExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

impl ProjectExecution for Project {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.fields.is_empty() || self.fields.len() > 4096 {
            return Err(invalid("projection requires bounded nonempty fields"));
        }
        let mut fields = Vec::with_capacity(self.fields.len());
        for projected in &self.fields {
            if projected.name.is_empty() || self.fields.iter().filter(|field| field.name == projected.name).count() != 1 {
                return Err(invalid("projection names must be nonempty and unique"));
            }
            let field = projected.field.field(input)?.with_name(&projected.name);
            fields.push(field);
        }
        Ok(Schema::new_with_metadata(fields, input.metadata().clone()))
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        let schema = Arc::new(self.output_schema(input.schema().as_ref())?);
        let mut columns = Vec::with_capacity(self.fields.len());
        for projected in &self.fields {
            check()?;
            // A nested projection must propagate parent Struct validity.
            let mut array = Arc::clone(projected.field.array(input)?);
            for depth in 1..projected.field.fields.len() {
                let parent = FieldPath {
                    fields: projected.field.fields[..depth].to_vec(),
                };
                let parent = parent.array(input)?;
                if let Some(nulls) = parent.nulls() {
                    let combined = arrow_buffer::NullBuffer::union(array.nulls(), Some(nulls));
                    let data = array
                        .to_data()
                        .into_builder()
                        .nulls(combined)
                        .build()
                        .map_err(|_| invalid("projected parent validity is invalid"))?;
                    array = arrow_array::make_array(data);
                }
            }
            columns.push(array);
        }
        RecordBatch::try_new(schema, columns).map_err(|_| invalid("projected batch is invalid"))
    }
}
