/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod execute;
mod fields;
pub(crate) mod plan;
mod query;

/* === Re-Exports === */
pub(crate) use execute::satisfies;
pub use execute::{ProjectedRow, ScanCursor, ScanOrder, ScanPage, ScanSource, collect};
pub use fields::ScalarProjection;
pub use plan::{
    AsOf, AsOfStrategy, Collected, Compare, Explain, Expr, Frame, GroupBy, GroupOutput, GroupRow,
    LastBy, Predicate, ScalarField, ScalarPath, Scan,
};
pub use query::{FrameQuery, FrameResult};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::reduce::{Accumulator, Agg, ScalarKind, ScalarValue};
use super::{ReadLimits, ReadView};
use crate::storage::frame::plan::{compare_iter, heap, invalid};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
// Memory Explanation And View Checks Read Runtime Layouts; Data Stays Portable.
impl Frame {
    pub fn check_view(&self, view: &ReadView) -> Result<()> {
        self.check()?;
        if view.execution_limits().is_none() {
            return Err(invalid("frame requires bounded saved view"));
        }
        for scan in &self.scans {
            view.check_kind(scan.selection.kind, &scan.selection, None)?;
        }
        Ok(())
    }

    pub fn explain(&self, page: ReadLimits) -> Result<Explain> {
        self.check()?;
        let overflow = || invalid("explained memory overflows");
        let value = std::mem::size_of::<ScalarValue>();
        let accumulator = std::mem::size_of::<Accumulator>();
        let max_rows = self.max_rows.get();
        let max_groups = self.max_groups.get();

        // Pages, Ordering Witnesses And Join Candidates Of The Widest Pass
        let row = |scan: usize| {
            self.scans[scan]
                .columns
                .len()
                .checked_mul(value)?
                .checked_add(std::mem::size_of::<ProjectedRow>())
        };
        let page_of = |scan: usize| {
            page.rows
                .get()
                .checked_mul(row(scan)?)?
                .checked_add(page.bytes.get())?
                .checked_add(std::mem::size_of::<ScanPage>())?
                .checked_add(row(scan)?)
        };
        let mut page_bytes = 0usize;
        for pass in execute::plan(self)? {
            let mut bytes = page_of(pass.source).ok_or_else(overflow)?;
            for &join in &pass.joins {
                let right = self.as_ofs[join].right;
                bytes = bytes
                    .checked_add(row(right).ok_or_else(overflow)?)
                    .ok_or_else(overflow)?;
                if !execute::reuses_left(&self.as_ofs[join]) {
                    bytes = bytes
                        .checked_add(page_of(right).ok_or_else(overflow)?)
                        .ok_or_else(overflow)?;
                }
            }
            page_bytes = page_bytes.max(bytes);
        }

        // Group State, Median Scratch And Output Per GroupBy
        let mut group_state_bytes = 0usize;
        let mut median_scratch_bytes = 0usize;
        let mut output_bytes = std::mem::size_of::<Collected>();
        let mut order_bytes = 0usize;
        for group_by in &self.group_bys {
            let groups = group_by.group_bound(max_groups);
            let accumulators = if group_by.last_by.is_some() {
                max_groups
            } else {
                groups
            };
            let aggs = group_by.aggs.len();
            let keys = group_by.keys.len();
            let outer = group_by.outer.as_ref().map_or(0, Vec::len);
            let per_group = (|| {
                let state = if group_by.last_by.is_some() { 0 } else { aggs };
                keys.checked_mul(value)?
                    .checked_add(state.checked_mul(accumulator)?)?
                    .checked_add(std::mem::size_of::<execute::Group>())?
                    .checked_add(2 * std::mem::size_of::<usize>())
            })()
            .ok_or_else(overflow)?;
            let members = if group_by.last_by.is_some() {
                aggs.checked_mul(accumulator)
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<execute::Member>()))
                    .and_then(|bytes| bytes.checked_mul(max_groups))
                    .ok_or_else(overflow)?
            } else {
                0
            };
            let state = groups
                .checked_mul(per_group)
                .and_then(|bytes| bytes.checked_add(members))
                .and_then(|bytes| bytes.checked_add(outer.checked_mul(accumulator)?))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<execute::Table>()))
                .ok_or_else(overflow)?;
            group_state_bytes = group_state_bytes.checked_add(state).ok_or_else(overflow)?;

            let medians = group_by
                .aggs
                .iter()
                .filter(|(_, agg)| *agg == Agg::Median)
                .count();
            let outer_medians = group_by
                .outer
                .iter()
                .flatten()
                .filter(|agg| **agg == Agg::Median)
                .count();
            let scratch = execute::median_slots(max_rows, accumulators)
                .and_then(|slots| slots.checked_mul(medians))
                .and_then(|slots| {
                    slots.checked_add(execute::median_slots(groups, 1)?.checked_mul(outer_medians)?)
                })
                .and_then(|slots| slots.checked_mul(value))
                .ok_or_else(overflow)?;
            median_scratch_bytes = median_scratch_bytes
                .checked_add(scratch)
                .ok_or_else(overflow)?;
            if medians > 0 || outer_medians > 0 {
                order_bytes = order_bytes.max(
                    max_rows
                        .max(groups)
                        .checked_mul(std::mem::size_of::<usize>())
                        .ok_or_else(overflow)?,
                );
            }

            let output = keys
                .checked_add(aggs)
                .and_then(|width| width.checked_mul(value))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<GroupRow>()))
                .and_then(|bytes| bytes.checked_mul(groups))
                .and_then(|bytes| bytes.checked_add(outer.checked_mul(value)?))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<GroupOutput>()))
                .ok_or_else(overflow)?;
            output_bytes = output_bytes.checked_add(output).ok_or_else(overflow)?;
        }
        median_scratch_bytes = median_scratch_bytes
            .checked_add(order_bytes)
            .ok_or_else(overflow)?;
        let text_bytes = if self.scans.iter().any(|scan| {
            scan.columns
                .iter()
                .any(|column| column.kind == ScalarKind::Text)
        }) {
            page.bytes.get()
        } else {
            0
        };
        group_state_bytes = group_state_bytes
            .checked_add(text_bytes)
            .ok_or_else(overflow)?;

        let peak_bytes = page_bytes
            .checked_add(group_state_bytes)
            .and_then(|bytes| bytes.checked_add(median_scratch_bytes))
            .and_then(|bytes| bytes.checked_add(output_bytes))
            .ok_or_else(overflow)?;
        Ok(Explain {
            peak_bytes,
            page_bytes,
            group_state_bytes,
            median_scratch_bytes,
            output_bytes,
            text_bytes,
        })
    }
}
impl GroupBy {
    pub(crate) fn group_bound(&self, max_groups: usize) -> usize {
        match &self.groups {
            Some(groups) => groups.len(),
            None if self.keys.is_empty() => 1,
            None => max_groups,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
