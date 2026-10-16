/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

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
use super::transform::{Transform, invalid};
use crate::storage::arrow::{ArrowBatch, RetainedMemory, resident_bytes, retain_native};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// A validated pipeline with one output schema fixed before any rows are emitted.
pub struct Pipeline {
    input: Arc<Schema>,
    output: Arc<Schema>,
    transforms: Vec<Transform>,
    slice: RowSlice,
}

/// Global output slicing state for page-local transforms.
pub struct StreamSlice {
    skipped: u64,
    emitted: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Pipeline {
    pub fn new(input: Arc<Schema>, transforms: Vec<Transform>, slice: RowSlice) -> Result<Self> {
        crate::storage::arrow::validate_schema(&input)?;
        if transforms.len() > 64 {
            return Err(invalid("too many transforms"));
        }
        let mut schema = input.as_ref().clone();
        for transform in &transforms {
            schema = transform.output_schema(&schema)?;
            crate::storage::arrow::validate_schema(&schema)?;
        }
        Ok(Self {
            input,
            output: Arc::new(schema),
            transforms,
            slice,
        })
    }

    pub fn output_schema(&self) -> Arc<Schema> {
        Arc::clone(&self.output)
    }

    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(crate::storage::arrow::schema_memory(&self.input))
            .saturating_add(crate::storage::arrow::schema_memory(&self.output))
            .saturating_add(self.transforms.memory_size())
    }

    /// Move Page-Local Work Ahead Of The First Global Transform. Final Slicing
    /// Belongs Exclusively To The Global Suffix.
    pub fn split_streaming_prefix(&self) -> Result<Option<(Self, Self)>> {
        let Some(index) = self
            .transforms
            .iter()
            .position(|transform| !transform.is_streaming())
        else {
            return Ok(None);
        };
        if index == 0
            || matches!(
                self.transforms[index],
                Transform::Aggregate(_) | Transform::Histogram(_)
            )
        {
            return Ok(None);
        }
        let prefix = Self::new(
            Arc::clone(&self.input),
            self.transforms[..index].to_vec(),
            RowSlice::default(),
        )?;
        let suffix = Self::new(
            prefix.output_schema(),
            self.transforms[index..].to_vec(),
            self.slice,
        )?;
        Ok(Some((prefix, suffix)))
    }

    pub fn native_working_memory(&self, input: &RecordBatch) -> Result<usize> {
        if !self.is_streaming() {
            return Err(invalid(
                "native page execution requires streaming transforms",
            ));
        }
        self.native_memory(input).map(|estimate| estimate.0)
    }

    pub fn execute_native_batch(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<RecordBatch> {
        if !self.is_streaming() || self.slice != RowSlice::default() {
            return Err(invalid(
                "native page prefix requires streaming transforms without output slicing",
            ));
        }
        self.check_input(input)?;
        check()?;
        self.apply(input.clone(), check, reserve)
    }

    /// Reserve Every Kernel Ask Before A Batch Can Mutate Stateful Consumers.
    pub fn working_memory(&self, input: &RecordBatch) -> Result<usize> {
        self.check_input(input)?;
        self.estimate(input.num_rows(), resident_bytes(input)?, false)
            .map(|estimate| estimate.0)
    }

    pub fn complete_memory(&self, input: &[RecordBatch]) -> Result<usize> {
        let mut rows = 0usize;
        let mut resident = 0usize;
        for batch in input {
            self.check_input(batch)?;
            rows = rows
                .checked_add(batch.num_rows())
                .ok_or_else(|| invalid("query row estimate overflow"))?;
            resident = resident
                .checked_add(resident_bytes(batch)?)
                .ok_or_else(|| invalid("query memory estimate overflow"))?;
        }
        self.complete_shape_memory(rows, resident)
    }

    pub(super) fn complete_shape_memory(&self, rows: usize, resident: usize) -> Result<usize> {
        let concatenate = resident
            .checked_mul(4)
            .and_then(|bytes| {
                bytes.checked_add(
                    crate::storage::arrow::schema_memory(&self.input).saturating_mul(4),
                )
            })
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or_else(|| invalid("query concat estimate overflow"))?;
        let execution = self.estimate(rows, resident, false)?.0;
        concatenate
            .checked_add(execution)
            .ok_or_else(|| invalid("query complete estimate overflow"))
    }

    pub(super) fn native_memory(&self, input: &RecordBatch) -> Result<(usize, usize, usize)> {
        self.check_input(input)?;
        self.estimate(input.num_rows(), resident_bytes(input)?, true)
    }

    fn estimate(
        &self,
        input_rows: usize,
        resident: usize,
        native_only: bool,
    ) -> Result<(usize, usize, usize)> {
        let mut schema = self.input.as_ref().clone();
        let mut rows = input_rows;
        let mut width = schema.fields().len().max(1);
        let mut schema_bytes = crate::storage::arrow::schema_memory(&schema);
        let mut maximum_rows = rows.max(1);
        for transform in &self.transforms {
            schema = transform.output_schema(&schema)?;
            width = width.max(schema.fields().len());
            schema_bytes = schema_bytes.max(crate::storage::arrow::schema_memory(&schema));
            if let Transform::Histogram(histogram) = transform {
                maximum_rows = maximum_rows.max(histogram.edges.len() + 1);
            }
        }
        // Selection And Projection Cannot Invent Variable-Length Payload Bytes.
        // Output Width Covers Repeated Columns; Exact Fractions Need Four Native
        // Words Per Original Scalar. Histogram Replaces Input With Fixed Cells.
        let ceiling = resident
            .checked_mul(width)
            .and_then(|bytes| bytes.checked_mul(4))
            .and_then(|bytes| {
                maximum_rows
                    .checked_mul(width)
                    .and_then(|cells| cells.checked_mul(256))
                    .and_then(|metadata| bytes.checked_add(metadata))
            })
            .and_then(|bytes| bytes.checked_add(schema_bytes.saturating_mul(4)))
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or_else(|| invalid("query native estimate overflow"))?;
        let mut total = 0usize;
        let mut schema = self.input.as_ref().clone();
        for transform in &self.transforms {
            let output = transform.output_schema(&schema)?;
            let output_rows = match transform {
                Transform::Histogram(histogram) => histogram.edges.len() + 1,
                Transform::Aggregate(_) => rows.max(1),
                Transform::Downsample(downsample) => rows.min(downsample.max_rows),
                _ => rows,
            };
            let working_rows = rows.max(output_rows).max(1);
            let working_width = output.fields().len().max(schema.fields().len()).max(1);
            let ask = ceiling
                .checked_mul(8)
                .and_then(|bytes| {
                    working_rows
                        .checked_mul(working_width)
                        .and_then(|cells| cells.checked_mul(512))
                        .and_then(|scratch| bytes.checked_add(scratch))
                })
                .and_then(|bytes| {
                    bytes.checked_add(
                        crate::storage::arrow::schema_memory(&output).saturating_mul(4),
                    )
                })
                .and_then(|bytes| bytes.checked_add(4096))
                .ok_or_else(|| invalid("query kernel estimate overflow"))?;
            total = total
                .checked_add(ask)
                .ok_or_else(|| invalid("query kernel sum overflows"))?;
            rows = output_rows;
            schema = output;
        }
        if !native_only {
            total = total
                .checked_add(
                    ceiling
                        .checked_mul(8)
                        .and_then(|bytes| bytes.checked_add(65536))
                        .ok_or_else(|| invalid("query encoding estimate overflow"))?,
                )
                .ok_or_else(|| invalid("query execution estimate overflow"))?;
        }
        Ok((total, rows, ceiling))
    }

    pub fn incremental(&self) -> Result<Option<super::reduction::Reduction>> {
        let Some(index) = self
            .transforms
            .iter()
            .position(|transform| !transform.is_streaming())
        else {
            return Ok(None);
        };
        let transform = &self.transforms[index];
        if !matches!(transform, Transform::Aggregate(_) | Transform::Histogram(_)) {
            return Ok(None);
        }
        let before = Self::new(
            Arc::clone(&self.input),
            self.transforms[..index].to_vec(),
            RowSlice::default(),
        )?;
        let middle = Arc::new(transform.output_schema(before.output_schema().as_ref())?);
        let after = Self::new(middle, self.transforms[index + 1..].to_vec(), self.slice)?;
        Ok(Some(super::reduction::Reduction::new(
            before,
            transform.clone(),
            after,
        )?))
    }

    pub fn is_streaming(&self) -> bool {
        self.transforms.iter().all(Transform::is_streaming)
    }

    pub fn stream_slice(&self) -> StreamSlice {
        StreamSlice {
            skipped: 0,
            emitted: 0,
        }
    }

    /// Execute global transforms over the complete selected source. The caller
    /// admits input pages before collecting them; every kernel admits coexistence.
    pub fn execute(
        &self,
        input: &[RecordBatch],
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<ArrowBatch> {
        check()?;
        for batch in input {
            self.check_input(batch)?;
        }
        let bytes = input.iter().try_fold(0usize, |total, batch| {
            total
                .checked_add(resident_bytes(batch)?)
                .ok_or_else(|| invalid("query input memory overflow"))
        })?;
        let reservation = reserve(
            bytes
                .checked_mul(4)
                .and_then(|size| {
                    size.checked_add(
                        crate::storage::arrow::schema_memory(&self.input).saturating_mul(4),
                    )
                })
                .and_then(|size| size.checked_add(65536))
                .ok_or_else(|| invalid("query concatenation memory overflow"))?,
        )?;
        let current = arrow_select::concat::concat_batches(&self.input, input.iter())
            .map_err(|_| invalid("query input concatenation failed"))?;
        let aliased = input.iter().collect::<Vec<_>>();
        reservation.shrink_to(crate::storage::arrow::new_resident_bytes(
            &current, &aliased,
        )?)?;
        let current = retain_native(current, reservation)?;
        let current = self.apply(current, check, reserve)?;
        let current = slice(&current, self.slice.offset, self.slice.limit)?;
        finish(current, check, reserve)
    }

    /// Filter and projection can run page by page without retaining all inputs.
    /// The state applies the final limit/offset across the whole invocation.
    pub fn execute_batch(
        &self,
        input: &RecordBatch,
        state: &mut StreamSlice,
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<ArrowBatch> {
        if !self.is_streaming() {
            return Err(invalid("global transform requires complete source"));
        }
        self.check_input(input)?;
        let current = self.apply(input.clone(), check, reserve)?;
        let remaining_offset = self.slice.offset.saturating_sub(state.skipped);
        state.skipped = state
            .skipped
            .checked_add((current.num_rows() as u64).min(remaining_offset))
            .ok_or_else(|| invalid("query offset overflow"))?;
        let remaining_limit = self
            .slice
            .limit
            .map(|limit| limit.saturating_sub(state.emitted));
        let current = slice(&current, remaining_offset, remaining_limit)?;
        state.emitted = state
            .emitted
            .checked_add(current.num_rows() as u64)
            .ok_or_else(|| invalid("query row count overflow"))?;
        finish(current, check, reserve)
    }

    pub fn is_finished(&self, state: &StreamSlice) -> bool {
        self.slice.limit.is_some_and(|limit| state.emitted >= limit)
    }

    pub(super) fn check_input(&self, batch: &RecordBatch) -> Result<()> {
        if batch.schema() != self.input {
            return Err(invalid("source batch schema changed"));
        }
        Ok(())
    }

    pub(super) fn apply(
        &self,
        mut current: RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<RecordBatch> {
        for transform in &self.transforms {
            check()?;
            let reservation = reserve(working_bound(&current, transform)?)?;
            let output = transform.apply(&current, check)?;
            let retained = crate::storage::arrow::new_resident_bytes(&output, &[&current])?;
            reservation.shrink_to(retained)?;
            current = retain_native(output, reservation)?;
        }
        Ok(current)
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(super) fn working_bound(input: &RecordBatch, transform: &Transform) -> Result<usize> {
    let resident = resident_bytes(input)?;
    let output = transform.output_schema(input.schema().as_ref())?;
    let width = output.fields().len().max(input.num_columns()).max(1);
    let rows = match transform {
        Transform::Histogram(value) => input.num_rows().max(value.edges.len() + 1),
        _ => input.num_rows().max(1),
    };
    // Shared scalar reducers retain exact values for order statistics. Include
    // input/output coexistence, grouping keys, ordering and nested owner metadata.
    resident
        .checked_mul(8)
        .and_then(|bytes| {
            rows.checked_mul(width)
                .and_then(|cells| cells.checked_mul(512))
                .and_then(|scratch| bytes.checked_add(scratch))
        })
        .and_then(|bytes| {
            bytes.checked_add(crate::storage::arrow::schema_memory(&output).saturating_mul(4))
        })
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or_else(|| invalid("query transform working memory overflow"))
}

pub(super) fn finish(
    batch: RecordBatch,
    check: &mut impl FnMut() -> Result<()>,
    reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
) -> Result<ArrowBatch> {
    check()?;
    // Uncompressed IPC needs bounded alignment and descriptor space in addition
    // to native buffers. Reserve before invoking its allocating writer.
    let resident = resident_bytes(&batch)?;
    let encoding = resident
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(65536))
        .ok_or_else(|| invalid("query encoding memory overflow"))?;
    let reservation = reserve(encoding)?;
    let output = ArrowBatch::try_new(batch)?;
    reservation.shrink_to(output.encoding_memory_size()?)?;
    let output = output.retain(reservation)?;
    check()?;
    Ok(output)
}

fn slice(batch: &RecordBatch, offset: u64, limit: Option<u64>) -> Result<RecordBatch> {
    let offset = usize::try_from(offset.min(batch.num_rows() as u64))
        .map_err(|_| invalid("query offset exceeds platform size"))?;
    let available = batch.num_rows() - offset;
    let rows = usize::try_from(limit.unwrap_or(available as u64).min(available as u64))
        .map_err(|_| invalid("query limit exceeds platform size"))?;
    Ok(batch.slice(offset, rows))
}

/* === Tests === */
