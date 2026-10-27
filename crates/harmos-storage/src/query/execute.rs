/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Query Plan Execution Over Arrow Batches

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;

// Local Imports
use super::RowSlice;
use super::spill::{Builder, Pages};
use super::transform::{Transform, TransformExecution, invalid};
use crate::Result;

/* === Constants === */

pub(super) const MAX_TRANSFORMS: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Plan {
    input: Arc<Schema>,
    output: Arc<Schema>,
    prefix: Vec<Transform>,
    global: Vec<Transform>,
    spool: Option<Builder>,
    pages: Option<Pages>,
    slice: RowSlice,
    skipped: u64,
    emitted: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Plan {
    #[tracing::instrument(name = "storage.query.execute.new", level = "trace", skip_all)]
    pub(super) fn new(input: Arc<Schema>, transforms: Vec<Transform>, slice: RowSlice) -> Result<Self> {
        crate::storage::arrow::validate_schema(&input)?;
        if transforms.len() > MAX_TRANSFORMS {
            return Err(invalid("too many query transforms"));
        }
        let mut schema = input.as_ref().clone();
        let mut stage = None;
        for (index, transform) in transforms.iter().enumerate() {
            if stage.is_none() && !transform.is_streaming() {
                stage = Some((index, Arc::new(schema.clone())));
            }
            schema = transform.output_schema(&schema)?;
            crate::storage::arrow::validate_schema(&schema)?;
        }
        let mut prefix = transforms;
        let (global, spool) = match stage {
            None => (Vec::new(), None),
            Some((index, schema)) => (prefix.split_off(index), Some(Builder::cached(schema)?)),
        };
        Ok(Self {
            input,
            output: Arc::new(schema),
            prefix,
            global,
            spool,
            pages: None,
            slice,
            skipped: 0,
            emitted: 0,
        })
    }

    pub(super) fn output_schema(&self) -> Arc<Schema> {
        Arc::clone(&self.output)
    }

    pub(super) fn is_satisfied(&self) -> bool {
        self.slice.limit.is_some_and(|limit| self.emitted >= limit)
    }

    #[tracing::instrument(name = "storage.query.execute.page", level = "trace", skip_all)]
    pub(super) fn page(&mut self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<Option<RecordBatch>> {
        if input.schema() != self.input {
            return Err(invalid("source batch schema changed"));
        }
        let current = apply(&self.prefix, input.clone(), check)?;
        if let Some(spool) = &mut self.spool {
            spool.push(&current, check)?;
            Ok(None)
        } else {
            Ok(Some(self.slice_output(&current)?))
        }
    }

    #[tracing::instrument(name = "storage.query.execute.finish", level = "trace", skip_all)]
    pub(super) fn finish(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<Option<RecordBatch>> {
        if let Some(spool) = self.spool.take() {
            let mut input = spool.finish()?;
            // Each Global Transform Sees The Complete Previous Result In Paged Form.
            for transform in &self.global {
                check()?;
                input = super::reduction::transform(input, transform, check)?;
            }
            self.pages = Some(input.pages()?);
        }
        if self.is_satisfied() {
            self.pages = None;
            return Ok(None);
        }
        loop {
            check()?;
            let Some(pages) = &mut self.pages else {
                return Ok(None);
            };
            let Some(batch) = pages.next()? else {
                self.pages = None;
                return Ok(None);
            };
            let batch = self.slice_output(&batch)?;
            if batch.num_rows() != 0 {
                return Ok(Some(batch));
            }
        }
    }

    // Apply The Invocation-Wide Offset And Limit To One Output Batch.
    fn slice_output(&mut self, batch: &RecordBatch) -> Result<RecordBatch> {
        let rows = batch.num_rows() as u64;
        let skip = self.slice.offset.saturating_sub(self.skipped).min(rows);
        self.skipped += skip;
        let available = rows - skip;
        let take = self.slice.limit.map_or(available, |limit| limit.saturating_sub(self.emitted).min(available));
        self.emitted += take;
        let offset = usize::try_from(skip).map_err(|_| invalid("query offset exceeds platform size"))?;
        let length = usize::try_from(take).map_err(|_| invalid("query limit exceeds platform size"))?;
        Ok(batch.slice(offset, length))
    }
}

/* === Public Functions === */

/* === Private Functions === */

// Run The Streaming Prefix In Source Order.
#[tracing::instrument(name = "storage.query.execute.apply", level = "trace", skip_all)]
fn apply(transforms: &[Transform], mut current: RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    for transform in transforms {
        check()?;

        let output = transform.apply(&current, check)?;
        current = output;
    }
    Ok(current)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::transform::{
        FieldPath,
        downsample::{Algorithm, Downsample},
        sort::{Sort, SortKey},
    };
    use arrow_array::{Float64Array, UInt64Array};
    use arrow_schema::{DataType, Field};

    fn source(length: usize) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("x", DataType::Float64, false),
            Field::new("y", DataType::Float64, false),
            Field::new("ordinal", DataType::UInt64, false),
        ]));
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Float64Array::from((0..length).map(|i| (i + 1) as f64).collect::<Vec<_>>())),
                Arc::new(Float64Array::from((0..length).map(|i| ((i * 17 % 101) as f64 - 50.0).sin()).collect::<Vec<_>>())),
                Arc::new(UInt64Array::from((0..length as u64).collect::<Vec<_>>())),
            ],
        )
        .unwrap()
    }

    fn run(batch: &RecordBatch, transforms: Vec<Transform>, slice: RowSlice) -> Vec<u64> {
        let mut plan = Plan::new(batch.schema(), transforms, slice).unwrap();
        let mut output = Vec::new();
        let mut collect = |batch: RecordBatch| {
            let column = batch.column_by_name("ordinal").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
            output.extend(column.values().iter().copied());
        };
        for offset in (0..batch.num_rows()).step_by(37) {
            let page = batch.slice(offset, 37.min(batch.num_rows() - offset));
            if let Some(batch) = plan.page(&page, &mut || Ok(())).unwrap() {
                collect(batch);
            }
        }
        while let Some(batch) = plan.finish(&mut || Ok(())).unwrap() {
            collect(batch);
        }
        output
    }

    #[test]
    fn spilled_reductions_preserve_seven_day_indices_and_slicing() {
        let batch = source(7 * 24 * 60);
        let x = batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values();
        let y = batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap().values();
        for algorithm in [Algorithm::Decimate, Algorithm::Extrema, Algorithm::Lttb, Algorithm::LttbLog] {
            let expected = match algorithm {
                Algorithm::Decimate => crate::signal::decimate(batch.num_rows(), 40),
                Algorithm::Extrema => crate::signal::extrema(y, 20),
                Algorithm::Lttb => crate::signal::lttb(x, y, 40),
                Algorithm::LttbLog => crate::signal::lttb_log(x, y, 40),
            };
            let transform = Transform::Downsample(Downsample {
                algorithm,
                max_rows: 40,
                value: Some(FieldPath::from("y")),
                axis: Some(FieldPath::from("x")),
            });
            let actual = run(&batch, vec![transform], RowSlice { offset: 2, limit: Some(17) });
            assert_eq!(actual, expected.into_iter().skip(2).take(17).map(|i| i as u64).collect::<Vec<_>>());
        }
    }

    #[test]
    fn multiple_global_stages_keep_stable_order_across_spill_runs() {
        let batch = source(350);
        let sort = Transform::Sort(Sort {
            keys: vec![SortKey {
                field: FieldPath::from("ordinal"),
                descending: true,
                nulls_first: false,
            }],
        });
        let downsample = Transform::Downsample(Downsample {
            algorithm: Algorithm::Decimate,
            max_rows: 20,
            value: None,
            axis: None,
        });
        let actual = run(&batch, vec![sort, downsample], RowSlice { offset: 0, limit: None });
        let expected = crate::signal::decimate(350, 20).into_iter().map(|i| 349 - i as u64).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn merged_spill_runs_keep_stable_ties() {
        let rows = 2 * super::super::spill::PAGE_ROWS + 1_000;
        let schema =
            Arc::new(Schema::new(vec![Field::new("key", DataType::UInt64, false), Field::new("ordinal", DataType::UInt64, false)]));
        let keys = (0..rows as u64).map(|row| row * 7_919 % 13).collect::<Vec<_>>();
        let batch = RecordBatch::try_new(
            schema,
            vec![Arc::new(UInt64Array::from(keys.clone())), Arc::new(UInt64Array::from((0..rows as u64).collect::<Vec<_>>()))],
        )
        .unwrap();
        let sort = Transform::Sort(Sort {
            keys: vec![SortKey {
                field: FieldPath::from("key"),
                descending: true,
                nulls_first: false,
            }],
        });
        let actual = run(&batch, vec![sort], RowSlice { offset: 0, limit: None });
        let mut expected = (0..rows as u64).collect::<Vec<_>>();
        expected.sort_by(|left, right| keys[*right as usize].cmp(&keys[*left as usize]));
        assert_eq!(actual, expected);
    }

    #[test]
    fn cancellation_refuses_spilled_global_work() {
        let batch = source(120);
        let mut plan = Plan::new(
            batch.schema(),
            vec![Transform::Downsample(Downsample {
                algorithm: Algorithm::Lttb,
                max_rows: 5,
                value: Some(FieldPath::from("y")),
                axis: None,
            })],
            RowSlice { offset: 0, limit: None },
        )
        .unwrap();
        plan.page(&batch, &mut || Ok(())).unwrap();
        assert!(plan.finish(&mut || Err(crate::Error::Closing { domain: "test cancellation" })).is_err());
    }
}
