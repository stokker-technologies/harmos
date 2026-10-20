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

use crate::storage::query::transform::AggregateExecution;
use crate::storage::query::transform::HistogramExecution;
use crate::storage::query::transform::TransformExecution;
// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;

// Local Imports
use super::RowSlice;
use super::transform::aggregate::Groups;
use super::transform::histogram::Bins;
use super::transform::{Transform, invalid};
use crate::storage::arrow::{RetainedMemory, resident_bytes, retain_native, schema_memory};
use crate::storage::memory::MemoryBank;
use crate::{MemorySize, Result};

/* === Constants === */

const MAX_TRANSFORMS: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Plan {
    input: Arc<Schema>,
    output: Arc<Schema>,
    prefix: Vec<Transform>,
    stage: Stage,
    suffix: Vec<Transform>,
    slice: RowSlice,
    skipped: u64,
    emitted: u64,
    retained: Vec<RetainedMemory>,
}

/* === Enums === */

enum Stage {
    Stream,
    Aggregate(Option<Groups>),
    Histogram(Option<Bins>),
    Collect(Transform, Vec<RecordBatch>),
}

/* === Traits === */

/* === Implementations === */

impl Plan {
    pub(super) fn new(input: Arc<Schema>, transforms: Vec<Transform>, slice: RowSlice) -> Result<Self> {
        crate::storage::arrow::validate_schema(&input)?;
        if transforms.len() > MAX_TRANSFORMS {
            return Err(invalid("too many query transforms"));
        }

        // Validate The Whole Chain Before Any Execution.
        let mut schema = input.as_ref().clone();
        let mut stage_input = None;
        for (index, transform) in transforms.iter().enumerate() {
            if stage_input.is_none() && !transform.is_streaming() {
                stage_input = Some((index, Arc::new(schema.clone())));
            }
            schema = transform.output_schema(&schema)?;
            crate::storage::arrow::validate_schema(&schema)?;
        }

        // Split Into Page Prefix, Global Stage And Suffix.
        let mut transforms = transforms;
        let (prefix, stage, suffix) = match stage_input {
            None => (transforms, Stage::Stream, Vec::new()),
            Some((index, stage_schema)) => {
                let suffix = transforms.split_off(index + 1);
                let transform = transforms.pop().expect("global stage transform");
                let stage = match &transform {
                    Transform::Aggregate(aggregate) => Stage::Aggregate(Some(aggregate.start(&stage_schema)?)),
                    Transform::Histogram(histogram) => Stage::Histogram(Some(histogram.start(&stage_schema)?)),
                    _ => Stage::Collect(transform, Vec::new()),
                };
                (transforms, stage, suffix)
            }
        };
        Ok(Self { input, output: Arc::new(schema), prefix, stage, suffix, slice, skipped: 0, emitted: 0, retained: Vec::new() })
    }

    pub(super) fn output_schema(&self) -> Arc<Schema> {
        Arc::clone(&self.output)
    }

    pub(super) fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(schema_memory(&self.input))
            .saturating_add(schema_memory(&self.output))
            .saturating_add(self.prefix.memory_size())
            .saturating_add(self.suffix.memory_size())
    }

    pub(super) fn is_satisfied(&self) -> bool {
        matches!(self.stage, Stage::Stream) && self.slice.limit.is_some_and(|limit| self.emitted >= limit)
    }

    pub(super) fn page(
        &mut self, input: &RecordBatch, bank: &MemoryBank, check: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<RecordBatch>> {
        if input.schema() != self.input {
            return Err(invalid("source batch schema changed"));
        }
        let current = apply(&self.prefix, input.clone(), bank, check)?;
        match &mut self.stage {
            Stage::Stream => {
                let current = self.slice_output(&current)?;
                Ok(Some(current))
            }
            Stage::Aggregate(groups) => {
                let groups = groups.as_mut().ok_or_else(|| invalid("finished reduction"))?;
                let transform_bound = working_bound(&current, &self.output)?;
                let _scratch = claim(bank, transform_bound)?;
                // The Growth Bound Is Claimed Before The Groups Change.
                let before = groups.retained_bytes();
                let growth = bank.reserve(transform_bound)?;
                groups.push(&current, check)?;
                let grown = groups.retained_bytes().saturating_sub(before);
                growth.shrink_to(grown.min(transform_bound))?;
                self.retained.push(growth);
                Ok(None)
            }
            Stage::Histogram(bins) => {
                let bins = bins.as_mut().ok_or_else(|| invalid("finished reduction"))?;
                if self.retained.is_empty() {
                    self.retained.push(bank.reserve(bins.retained_bytes())?);
                }
                let _scratch = claim(bank, bins.retained_bytes())?;
                bins.push(&current, check)?;
                Ok(None)
            }
            Stage::Collect(_, batches) => {
                let retained = bank.reserve(resident_bytes(&current)?)?;
                let current = retain_native(current, retained)?;
                batches.push(current);
                Ok(None)
            }
        }
    }

    pub(super) fn finish_bytes(&self) -> Result<usize> {
        let stage = match &self.stage {
            Stage::Stream => 0,
            Stage::Aggregate(groups) => groups.as_ref().map_or(Ok(0), Groups::finish_bytes)?,
            Stage::Histogram(bins) => bins.as_ref().map_or(0, Bins::finish_bytes),
            Stage::Collect(_, batches) => batches.iter().try_fold(0usize, |total, batch| {
                resident_bytes(batch)?
                    .checked_mul(8)
                    .and_then(|bytes| total.checked_add(bytes))
                    .ok_or_else(|| invalid("query collection estimate overflow"))
            })?,
        };
        stage
            .checked_mul(1 + self.suffix.len())
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or_else(|| invalid("query finish estimate overflow"))
    }

    pub(super) fn finish(&mut self, bank: &MemoryBank, check: &mut impl FnMut() -> Result<()>) -> Result<Option<RecordBatch>> {
        let output = match &mut self.stage {
            Stage::Stream => return Ok(None),
            Stage::Aggregate(groups) => {
                let Some(groups) = groups.take() else {
                    return Ok(None);
                };
                let _scratch = claim(bank, groups.finish_bytes()?)?;
                groups.finish(check)?
            }
            Stage::Histogram(bins) => {
                let Some(bins) = bins.take() else {
                    return Ok(None);
                };
                let _scratch = claim(bank, bins.finish_bytes())?;
                bins.finish()?
            }
            Stage::Collect(transform, batches) => {
                let inputs = std::mem::take(batches);
                let schema = inputs.first().map_or_else(
                    || Ok(Arc::new(transform_input(&self.input, &self.prefix)?)),
                    |batch| Ok::<_, crate::Error>(batch.schema()),
                )?;
                let resident =
                    inputs.iter().try_fold(0usize, |total, batch| resident_bytes(batch).map(|bytes| total.saturating_add(bytes)))?;
                let _concat = claim(bank, resident.saturating_mul(2).saturating_add(65536))?;
                let combined = arrow_select::concat::concat_batches(&schema, inputs.iter())
                    .map_err(|_| invalid("query input concatenation failed"))?;
                drop(inputs);
                let _work = claim(bank, working_bound(&combined, &self.output)?)?;
                transform.apply(&combined, check)?
            }
        };
        self.retained.clear();
        let output = apply(&self.suffix, output, bank, check)?;
        let output = self.slice_output(&output)?;
        let retained = bank.reserve(resident_bytes(&output)?)?;
        retain_native(output, retained).map(Some)
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

pub(super) fn working_bound(input: &RecordBatch, output: &Schema) -> Result<usize> {
    let resident = resident_bytes(input)?;
    let width = output.fields().len().max(input.num_columns()).max(1);
    let rows = input.num_rows().max(1);
    resident
        .checked_mul(8)
        .and_then(|bytes| rows.checked_mul(width).and_then(|cells| cells.checked_mul(512)).and_then(|scratch| bytes.checked_add(scratch)))
        .and_then(|bytes| bytes.checked_add(schema_memory(output).saturating_mul(4)))
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or_else(|| invalid("query transform working memory overflow"))
}

/* === Private Functions === */

// Run Transforms In Order, Retaining Each Output Under The Attempt Bank.
fn apply(
    transforms: &[Transform], mut current: RecordBatch, bank: &MemoryBank, check: &mut impl FnMut() -> Result<()>,
) -> Result<RecordBatch> {
    for transform in transforms {
        check()?;
        let output_schema = transform.output_schema(current.schema().as_ref())?;
        let _scratch = claim(bank, working_bound(&current, &output_schema)?)?;
        let output = transform.apply(&current, check)?;
        let retained = bank.reserve(crate::storage::arrow::new_resident_bytes(&output, &[&current])?)?;
        current = retain_native(output, retained)?;
    }
    Ok(current)
}

fn claim(bank: &MemoryBank, bytes: usize) -> Result<crate::storage::memory::MemoryScope<'_>> {
    let mut scope = bank.scope();
    scope.claim(bytes)?;
    Ok(scope)
}

fn transform_input(input: &Schema, prefix: &[Transform]) -> Result<Schema> {
    prefix.iter().try_fold(input.clone(), |schema, transform| transform.output_schema(&schema))
}

/* === Tests === */
