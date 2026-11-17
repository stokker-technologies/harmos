/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Deterministic Statistics Integration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use arrow_array::BinaryArray;

// Local Imports
use super::{buckets, equal_envelopes, input};
use harmos_query::transform::{Transform, TransformExecution};
use harmos_query::{RowSlice, execute::Plan};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn bucket_continuation_equals_whole_range_across_pages_and_bounds() {
    let values = (0..48).map(|index| ((index * 13) % 23) as f64 - 11.0).collect();
    let batch = input(values, 1);
    for bound in [1, 2, 5, 64] {
        let transform = Transform::BucketSummary(buckets(48, bound));
        let whole = transform.apply(&batch, &mut || Ok(())).unwrap();
        assert!(whole.num_rows() <= bound);
        let first = Transform::BucketSummary(buckets(24, bound)).apply(&batch.slice(0, 24), &mut || Ok(())).unwrap();
        let mut continued = buckets(48, bound);
        continued.prior =
            Some(first.column_by_name("continuation").unwrap().as_any().downcast_ref::<BinaryArray>().unwrap().value(0).to_vec());
        let continued = Transform::BucketSummary(continued);
        let actual = continued.apply(&batch.slice(24, 24), &mut || Ok(())).unwrap();
        equal_envelopes(&whole, &actual);
        assert!(continued.apply(&batch.slice(0, 1), &mut || Ok(())).is_err());
        let mut plan = Plan::new(batch.schema(), vec![transform], RowSlice::default()).unwrap();
        for offset in (0..48).step_by(7) {
            assert!(plan.page(&batch.slice(offset, (48 - offset).min(7)), &mut || Ok(())).unwrap().is_none());
        }
        let paged = plan.finish(&mut || Ok(())).unwrap().unwrap();
        equal_envelopes(&whole, &paged);
    }
}
