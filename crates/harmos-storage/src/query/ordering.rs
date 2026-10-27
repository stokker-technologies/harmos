/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stable Bounded External Row Ordering

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;

// Third Party Imports
use arrow_array::RecordBatch;

// Local Imports
use super::accumulate::sort_values;
use super::scalar::ScalarValue;
use super::spill::{Builder, Cursor, Rows, Spool};
use super::transform::sort::Sort;
use super::transform::{FieldPathExecution, invalid};
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn sort(input: Spool, plan: &Sort, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    ordered(input, &mut |left, a, right, b| compare(plan, left, a, right, b), check)
}

pub(super) fn ordered(
    input: Spool, compare: &mut impl FnMut(&RecordBatch, usize, &RecordBatch, usize) -> Result<Ordering>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Spool> {
    let mut levels: Vec<Option<Spool>> = Vec::new();
    let mut reader = input.reader()?;
    for batch in &mut reader {
        check()?;
        let batch = batch.map_err(|_| invalid("sort spill decoding failed"))?;
        let mut indices: Vec<u64> = (0..batch.num_rows() as u64).collect();
        sort_values(&mut indices, |a, b| {
            check()?;
            Ok(compare(&batch, a as usize, &batch, b as usize)?.then(a.cmp(&b)))
        })?;
        let sorted = super::transform::take(&batch, indices)?;
        let mut writer = Builder::new(input.schema.clone())?;
        writer.push(&sorted, check)?;
        let mut run = writer.finish()?;
        let mut level = 0;
        loop {
            if level == levels.len() {
                levels.push(None);
            }
            match levels[level].take() {
                None => {
                    levels[level] = Some(run);
                    break;
                }
                Some(older) => run = merge(older, run, compare, check)?,
            }
            level += 1;
        }
    }
    let mut output = None;
    for run in levels.into_iter().flatten() {
        output = Some(match output {
            None => run,
            Some(newer) => merge(run, newer, compare, check)?,
        });
    }
    match output {
        Some(output) => Ok(output),
        None => Builder::new(input.schema.clone())?.finish(),
    }
}

/* === Private Functions === */

fn merge(
    left: Spool, right: Spool, compare: &mut impl FnMut(&RecordBatch, usize, &RecordBatch, usize) -> Result<Ordering>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Spool> {
    let mut output = Rows::new(left.schema.clone())?;
    let mut left = Cursor::new(left)?;
    let mut right = Cursor::new(right)?;
    while left.batch.is_some() || right.batch.is_some() {
        check()?;
        let choose_left = match (&left.batch, &right.batch) {
            (Some(a), Some(b)) => !compare(a, left.row, b, right.row)?.is_gt(),
            (Some(_), None) => true,
            _ => false,
        };
        let cursor = if choose_left { &mut left } else { &mut right };
        output.push(cursor.batch.as_ref().expect("nonempty merge side"), cursor.row, check)?;
        cursor.advance()?;
    }
    output.finish(check)
}

pub(super) fn compare(plan: &Sort, left: &RecordBatch, a: usize, right: &RecordBatch, b: usize) -> Result<Ordering> {
    for key in &plan.keys {
        let left = key.field.value(left, a)?;
        let right = key.field.value(right, b)?;
        let absent = |value: &ScalarValue| {
            matches!(value, ScalarValue::Null | ScalarValue::Missing) || matches!(value, ScalarValue::Float(value) if !value.is_finite())
        };
        let order = match (absent(&left), absent(&right)) {
            (true, true) => Ordering::Equal,
            (true, false) => {
                if key.nulls_first {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (false, true) => {
                if key.nulls_first {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (false, false) => {
                let order = left.compare(&right)?;
                if key.descending { order.reverse() } else { order }
            }
        };
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

/* === Tests === */
