/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Immutable Paged Query Tables For Derived Evidence

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;

// Local Imports
use super::spill::{Builder, Pages, Spool};
use super::transform::{Transform, TransformExecution, invalid};
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Immutable Derived Rows, Retained As Bounded Arrow Pages Or Private Spill Files.
#[derive(Clone)]
pub struct Table {
    spool: Spool,
}

/// Appends Producer Pages Without Retaining Earlier Pages.
pub struct TableWriter {
    builder: Builder,
    schema: Arc<Schema>,
}

/// Retains Shared Arrow Pages Or The Spill File Until The Reader Is Dropped.
pub struct TableReader {
    pages: Pages,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl TableWriter {
    pub fn new(schema: Arc<Schema>) -> Result<Self> {
        crate::storage::arrow::validate_schema(&schema)?;
        let builder = Builder::new(schema.clone())?;
        Ok(Self { builder, schema })
    }

    pub fn push(&mut self, page: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        if page.schema() != self.schema {
            return Err(invalid("derived table schema changed"));
        }
        self.builder.push(page, check)
    }

    pub fn finish(self) -> Result<Table> {
        Ok(Table { spool: self.builder.finish()? })
    }
}

impl Table {
    pub fn len(&self) -> usize {
        self.spool.rows
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn schema(&self) -> Arc<Schema> {
        self.spool.schema.clone()
    }

    /// Apply One Transform To The Complete Table With Bounded Spill Workspace.
    pub fn transform(&self, transform: &Transform, check: &mut impl FnMut() -> Result<()>) -> Result<Self> {
        check()?;
        let schema = transform.output_schema(&self.spool.schema)?;
        crate::storage::arrow::validate_schema(&schema)?;
        let spool = super::reduction::transform(self.spool.clone(), transform, check)?;
        Ok(Self { spool })
    }

    pub fn reader(&self) -> Result<TableReader> {
        Ok(TableReader { pages: self.spool.clone().pages()? })
    }
}

impl TableReader {
    pub fn next(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<Option<RecordBatch>> {
        check()?;
        self.pages.next()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::super::transform::{
        FieldPath,
        downsample::{Algorithm, Downsample},
    };
    use super::*;
    use arrow_array::Float64Array;
    use arrow_schema::{DataType, Field};

    #[test]
    fn derived_table_reduces_exactly_across_pages_and_survives_owner_drop() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false), Field::new("y", DataType::Float64, false)]));
        let mut writer = TableWriter::new(schema.clone()).unwrap();
        let x: Vec<f64> = (0..10_080).map(f64::from).collect();
        let y: Vec<f64> = x.iter().map(|x| (x * 0.13).sin()).collect();
        for (x, y) in x.chunks(120).zip(y.chunks(120)) {
            let page = RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(Float64Array::from(x.to_vec())), Arc::new(Float64Array::from(y.to_vec()))],
            )
            .unwrap();
            writer.push(&page, &mut || Ok(())).unwrap();
        }
        let table = writer.finish().unwrap();
        assert_eq!(table.len(), x.len());
        for (algorithm, expected) in [
            (Algorithm::Lttb, crate::signal::lttb(&x, &y, 31)),
            (Algorithm::Extrema, crate::signal::extrema(&y, 15)),
            (Algorithm::Decimate, crate::signal::decimate(x.len(), 31)),
        ] {
            let transform = Transform::Downsample(Downsample {
                max_rows: 31,
                algorithm,
                value: Some(FieldPath { fields: vec!["y".into()] }),
                axis: Some(FieldPath { fields: vec!["x".into()] }),
            });
            let reduced = table.transform(&transform, &mut || Ok(())).unwrap();
            let mut reader = reduced.reader().unwrap();
            drop(reduced);
            let mut actual = Vec::new();
            while let Some(page) = reader.next(&mut || Ok(())).unwrap() {
                let values = page.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
                actual.extend(values.values().iter().map(|value| *value as usize));
            }
            assert_eq!(actual, expected);
        }
        let mut reader = table.reader().unwrap();
        drop(table);
        let mut rows = 0;
        while let Some(page) = reader.next(&mut || Ok(())).unwrap() {
            assert!(page.num_rows() <= super::super::spill::PAGE_ROWS);
            rows += page.num_rows();
        }
        assert_eq!(rows, x.len());
    }

    #[test]
    fn derived_table_rejects_changed_schema_and_polls_cancellation() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)]));
        let mut writer = TableWriter::new(schema.clone()).unwrap();
        let wrong = RecordBatch::new_empty(Arc::new(Schema::empty()));
        assert!(writer.push(&wrong, &mut || Ok(())).is_err());
        let page = RecordBatch::new_empty(schema);
        assert!(writer.push(&page, &mut || Err(invalid("cancelled"))).is_err());
        let table = writer.finish().unwrap();
        assert!(table.is_empty());
        let mut reader = table.reader().unwrap();
        assert!(reader.next(&mut || Err(invalid("cancelled"))).is_err());
        assert!(reader.next(&mut || Ok(())).unwrap().is_none());
    }
}
