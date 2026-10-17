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
use std::collections::BTreeMap;
use std::ptr::NonNull;
use std::sync::Arc;
// Third Party Imports
use arrow_array::{RecordBatch, make_array};
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer};
use arrow_data::ArrayData;
use arrow_schema::Schema;
// Local Imports
use super::{RetainedMemory, invalid};
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct Owner {
    pub(super) _reservation: RetainedMemory,
    pub(super) _encoded: Arc<Vec<u8>>,
    pub(super) _schema: Arc<Schema>,
}

struct BufferOwner {
    _buffer: Buffer,
    _owner: Arc<Owner>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn resident_bytes(batch: &RecordBatch) -> Result<usize> {
    let mut seen = BTreeMap::new();
    let mut bytes = super::schema_memory(batch.schema().as_ref());
    for column in batch.columns() {
        count(&column.to_data(), &mut seen, &mut bytes)?;
    }
    Ok(bytes)
}

pub(super) fn new_resident_bytes(batch: &RecordBatch, input: &[&RecordBatch]) -> Result<usize> {
    let mut seen = BTreeMap::new();
    let mut ignored = 0;
    for batch in input {
        for column in batch.columns() {
            count(&column.to_data(), &mut seen, &mut ignored)?;
        }
    }
    let mut bytes = super::schema_memory(batch.schema().as_ref());
    for column in batch.columns() {
        count(&column.to_data(), &mut seen, &mut bytes)?;
    }
    bytes.checked_add(owner_bytes(batch)?).ok_or_else(|| invalid("Arrow ownership size overflow"))
}

pub(super) fn owner_bytes(batch: &RecordBatch) -> Result<usize> {
    fn count_buffers(data: &ArrayData) -> Option<usize> {
        data.child_data()
            .iter()
            .try_fold(data.buffers().len() + usize::from(data.nulls().is_some()), |count, child| count.checked_add(count_buffers(child)?))
    }
    let buffers = batch
        .columns()
        .iter()
        .try_fold(0usize, |count, column| count.checked_add(count_buffers(&column.to_data())?))
        .ok_or_else(|| invalid("Arrow owner count overflow"))?;
    // Include Arc control blocks, custom Buffer scaffolding and column owners.
    buffers
        .checked_mul(std::mem::size_of::<BufferOwner>() + 256)
        .and_then(|bytes| bytes.checked_add(batch.num_columns().checked_mul(128)?))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Owner>() + 512))
        .ok_or_else(|| invalid("Arrow owner size overflow"))
}

pub(super) fn attach(batch: &RecordBatch, owner: Arc<Owner>) -> Result<RecordBatch> {
    let mut columns = Vec::with_capacity(batch.num_columns());
    for column in batch.columns() {
        columns.push(make_array(attach_data(column.to_data(), &owner)?));
    }
    RecordBatch::try_new_with_options(
        batch.schema(),
        columns,
        &arrow_array::RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )
    .map_err(|_| invalid("retained Arrow batch validation failed"))
}

/* === Private Functions === */
fn count(data: &ArrayData, seen: &mut BTreeMap<usize, usize>, bytes: &mut usize) -> Result<()> {
    *bytes = bytes.checked_add(std::mem::size_of::<ArrayData>()).ok_or_else(|| invalid("Arrow resident size overflow"))?;
    for buffer in data.buffers().iter().chain(data.nulls().map(|nulls| nulls.buffer())) {
        let base =
            (buffer.as_ptr() as usize).checked_sub(buffer.ptr_offset()).ok_or_else(|| invalid("Arrow allocation offset underflow"))?;
        // Arrow Custom Allocations Report Capacity Zero. The Accessible Span
        // Still Bounds Kernel Reads/Copies; Its Original Full Allocation Remains
        // Charged By BufferOwner Until The Last Alias Drops.
        let span = buffer.len().checked_add(buffer.ptr_offset()).ok_or_else(|| invalid("Arrow accessible allocation span overflow"))?;
        let capacity = buffer.capacity().max(span);
        let previous = seen.entry(base).or_insert(0);
        if capacity > *previous {
            *bytes = bytes.checked_add(capacity - *previous).ok_or_else(|| invalid("Arrow resident size overflow"))?;
            *previous = capacity;
        }
    }
    for child in data.child_data() {
        count(child, seen, bytes)?;
    }
    Ok(())
}

fn attach_data(data: ArrayData, owner: &Arc<Owner>) -> Result<ArrayData> {
    let buffers = data.buffers().iter().map(|buffer| attach_buffer(buffer, owner)).collect();
    let children = data.child_data().iter().map(|child| attach_data(child.clone(), owner)).collect::<Result<Vec<_>>>()?;
    let nulls = data.nulls().map(|nulls| {
        let boolean = nulls.inner();
        let buffer = attach_buffer(boolean.inner(), owner);
        NullBuffer::new(BooleanBuffer::new(buffer, boolean.offset(), boolean.len()))
    });
    data.into_builder()
        .buffers(buffers)
        .child_data(children)
        .nulls(nulls)
        .build()
        .map_err(|_| invalid("retained Arrow array validation failed"))
}

fn attach_buffer(buffer: &Buffer, owner: &Arc<Owner>) -> Buffer {
    let retained = Arc::new(BufferOwner { _buffer: buffer.clone(), _owner: Arc::clone(owner) });
    let pointer = NonNull::new(buffer.as_ptr().cast_mut()).expect("Arrow buffer pointer is nonnull");
    // SAFETY: BufferOwner retains this exact immutable source Buffer, hence its
    // allocation, for the custom buffer's entire lifetime. The source pointer
    // is valid for buffer.len() bytes; clones/slices retain the custom owner.
    // A zero-length slice can still retain its source's allocation, so it must
    // retain the reservation too. Arrow also provides a nonnull empty pointer.
    #[expect(unsafe_code, reason = "Arrow custom allocation retains the original immutable buffer and its reservation")]
    unsafe {
        Buffer::from_custom_allocation(pointer, buffer.len(), retained)
    }
}

/* === Tests === */
