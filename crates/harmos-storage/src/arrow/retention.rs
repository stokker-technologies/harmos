/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

use std::collections::BTreeMap;

use arrow_array::RecordBatch;
use arrow_data::ArrayData;

use super::invalid;
use crate::Result;

pub(super) fn resident_bytes(batch: &RecordBatch) -> Result<usize> {
    let mut seen = BTreeMap::new();
    let mut bytes = super::schema_memory(batch.schema().as_ref());
    for column in batch.columns() {
        count(&column.to_data(), &mut seen, &mut bytes)?;
    }
    Ok(bytes)
}

fn count(data: &ArrayData, seen: &mut BTreeMap<usize, usize>, bytes: &mut usize) -> Result<()> {
    *bytes = bytes.checked_add(std::mem::size_of::<ArrayData>()).ok_or_else(|| invalid("Arrow resident size overflow"))?;
    for buffer in data.buffers().iter().chain(data.nulls().map(|nulls| nulls.buffer())) {
        let base =
            (buffer.as_ptr() as usize).checked_sub(buffer.ptr_offset()).ok_or_else(|| invalid("Arrow allocation offset underflow"))?;
        // Include Accessible Spans When Custom Buffers Report Zero Capacity.
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
