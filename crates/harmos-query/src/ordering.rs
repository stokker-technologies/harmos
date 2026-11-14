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
    let mut key = |batch: &RecordBatch, row| plan.keys.iter().map(|key| key.field.value(batch, row)).collect::<Result<Vec<_>>>();
    ordered(input, &mut key, &mut |left, right| compare(plan, left, right), check)
}

// Each Row's Key Is Read Once Per Page Or Merge Step; Comparisons Never Revisit The Batch.
pub(super) fn ordered<K>(
    input: Spool, key: &mut impl FnMut(&RecordBatch, usize) -> Result<K>, compare: &mut impl FnMut(&K, &K) -> Result<Ordering>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Spool> {
    let mut levels: Vec<Option<Spool>> = Vec::new();
    let mut reader = input.reader()?;
    for batch in &mut reader {
        check()?;
        let batch = batch.map_err(|_| invalid("sort spill decoding failed"))?;
        let mut run = sorted_run(&input, &batch, key, compare, check)?;
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
                Some(older) => run = merge(older, run, key, compare, check)?,
            }
            level += 1;
        }
    }

    let mut output = None;
    for run in levels.into_iter().flatten() {
        output = Some(match output {
            None => run,
            Some(newer) => merge(run, newer, key, compare, check)?,
        });
    }
    match output {
        Some(output) => Ok(output),
        None => Builder::new(input.schema.clone())?.finish(),
    }
}

/* === Private Functions === */

fn sorted_run<K>(
    input: &Spool, batch: &RecordBatch, key: &mut impl FnMut(&RecordBatch, usize) -> Result<K>,
    compare: &mut impl FnMut(&K, &K) -> Result<Ordering>, check: &mut impl FnMut() -> Result<()>,
) -> Result<Spool> {
    let mut keys = Vec::new();
    keys.try_reserve_exact(batch.num_rows()).map_err(|_| invalid("sort key allocation failed"))?;
    for row in 0..batch.num_rows() {
        check()?;
        keys.push(key(batch, row)?);
    }
    let mut indices: Vec<u64> = (0..batch.num_rows() as u64).collect();
    sort_values(&mut indices, |a, b| {
        check()?;
        Ok(compare(&keys[a as usize], &keys[b as usize])?.then(a.cmp(&b)))
    })?;
    drop(keys);

    let sorted = super::transform::take(batch, indices)?;
    let mut writer = Builder::new(input.schema.clone())?;
    writer.push(&sorted, check)?;
    writer.finish()
}

fn merge<K>(
    left: Spool, right: Spool, key: &mut impl FnMut(&RecordBatch, usize) -> Result<K>,
    compare: &mut impl FnMut(&K, &K) -> Result<Ordering>, check: &mut impl FnMut() -> Result<()>,
) -> Result<Spool> {
    let mut output = Rows::new(left.schema.clone())?;
    let mut left = Cursor::new(left)?;
    let mut right = Cursor::new(right)?;
    let mut left_key = current_key(&left, key)?;
    let mut right_key = current_key(&right, key)?;
    loop {
        check()?;
        let choose_left = match (&left_key, &right_key) {
            (Some(a), Some(b)) => !compare(a, b)?.is_gt(),
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        if choose_left {
            output.push(&mut left, check)?;
            left.advance()?;
            left_key = current_key(&left, key)?;
        } else {
            output.push(&mut right, check)?;
            right.advance()?;
            right_key = current_key(&right, key)?;
        }
    }
    output.finish(check)
}

fn current_key<K>(cursor: &Cursor, key: &mut impl FnMut(&RecordBatch, usize) -> Result<K>) -> Result<Option<K>> {
    cursor.batch.as_ref().map(|batch| key(batch, cursor.row)).transpose()
}

fn compare(plan: &Sort, left: &[ScalarValue], right: &[ScalarValue]) -> Result<Ordering> {
    for ((key, left), right) in plan.keys.iter().zip(left).zip(right) {
        let absent = |value: &ScalarValue| {
            matches!(value, ScalarValue::Null | ScalarValue::Missing) || matches!(value, ScalarValue::Float(value) if !value.is_finite())
        };
        let order = match (absent(left), absent(right)) {
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
                let order = left.compare(right)?;
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
