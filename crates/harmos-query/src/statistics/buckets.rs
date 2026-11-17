/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Mergeable Ordered Points and Bounded Adaptive Envelopes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::mem;

// Third Party Imports

// Local Imports
use super::Summary;
use crate::Result;
use crate::transform::invalid;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::StorageCursor;
use harmos_domain::model::storage::query::transform::statistics::BucketSummary;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Point {
    pub time_ns: i64,
    pub position: u64,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct OrderedSummary {
    pub moments: Summary,
    pub first: Point,
    pub last: Point,
    pub min: Point,
    pub max: Point,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Buckets {
    plan: BucketSummary,
    pub width_ns: u128,
    pub buckets: BTreeMap<u64, OrderedSummary>,
    previous_cut: Option<StorageCursor>,
    last_position: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OrderedSummary {
    pub fn new(point: Point) -> Self {
        let mut moments = Summary::new();
        moments.push(point.value);
        Self {
            moments,
            first: point,
            last: point,
            min: point,
            max: point,
        }
    }
    pub fn merge(&mut self, other: &Self) {
        self.moments.merge(&other.moments);
        if (other.first.time_ns, other.first.position) < (self.first.time_ns, self.first.position) {
            self.first = other.first;
        }
        if (other.last.time_ns, other.last.position) > (self.last.time_ns, self.last.position) {
            self.last = other.last;
        }
        let lower = other
            .min
            .value
            .total_cmp(&self.min.value)
            .then((other.min.time_ns, other.min.position).cmp(&(self.min.time_ns, self.min.position)));
        let higher = other
            .max
            .value
            .total_cmp(&self.max.value)
            .reverse()
            .then((other.max.time_ns, other.max.position).cmp(&(self.max.time_ns, self.max.position)));
        if lower.is_lt() {
            self.min = other.min;
        }
        if higher.is_lt() {
            self.max = other.max;
        }
    }
}

impl Buckets {
    pub fn new(plan: &BucketSummary, check: &mut impl FnMut() -> Result<()>) -> Result<Self> {
        check()?;
        let owned = BucketSummary {
            time: plan.time.clone(),
            position: plan.position.clone(),
            value: plan.value.clone(),
            width_ns: plan.width_ns,
            max_buckets: plan.max_buckets,
            source_cut: plan.source_cut.clone(),
            prior: None,
        };
        let Some(prior) = &plan.prior else {
            return Ok(Self {
                width_ns: u128::from(plan.width_ns.get()),
                plan: owned,
                buckets: BTreeMap::new(),
                previous_cut: None,
                last_position: None,
            });
        };
        let maximum = plan
            .max_buckets
            .get()
            .checked_mul(1024)
            .and_then(|size| size.checked_add(owned.memory_size()))
            .ok_or_else(|| invalid("reduce the bucket bound"))?;
        if prior.len() > maximum {
            return Err(invalid("supply a continuation within the bucket bound"));
        }
        let state: Self = postcard::from_bytes(prior).map_err(|_| invalid("supply an intact bucket continuation"))?;
        let compatible = state.plan.time == plan.time
            && state.plan.position == plan.position
            && state.plan.value == plan.value
            && state.plan.width_ns == plan.width_ns
            && state.plan.max_buckets == plan.max_buckets
            && state.plan.source_cut.history == plan.source_cut.history
            && state.plan.source_cut.sequence <= plan.source_cut.sequence;
        let base = u128::from(plan.width_ns.get());
        let ratio = state.width_ns.checked_div(base).unwrap_or(0);
        let width_valid =
            state.width_ns >= base && state.width_ns.is_multiple_of(base) && ratio.is_power_of_two() && state.width_ns < (1_u128 << 65);
        if !compatible || !width_valid || state.buckets.len() > plan.max_buckets.get() {
            return Err(invalid("continue with the same fields, width, bucket bound and source history at a nondecreasing cut"));
        }
        let mut count = 0_u64;
        for (ordinal, summary) in &state.buckets {
            check()?;
            count = count.checked_add(summary.moments.count()).ok_or_else(|| invalid("supply an intact bucket continuation count"))?;
            if !summary.moments.is_valid()
                || count > state.plan.source_cut.sequence
                || summary.moments.min() != Some(summary.min.value)
                || summary.moments.max() != Some(summary.max.value)
                || (summary.first.time_ns, summary.first.position) > (summary.last.time_ns, summary.last.position)
                || [summary.first, summary.last, summary.min, summary.max].iter().any(|point| {
                    !point.value.is_finite()
                        || point.position == 0
                        || point.position > state.plan.source_cut.sequence
                        || ordinal_of(point.time_ns, state.width_ns) != *ordinal
                })
            {
                return Err(invalid("supply an intact bucket continuation with valid source points"));
            }
        }
        let previous_cut = Some(state.plan.source_cut.clone());
        let mut result = Self {
            width_ns: u128::from(plan.width_ns.get()),
            plan: owned,
            buckets: BTreeMap::new(),
            previous_cut,
            last_position: None,
        };
        result.merge(state, check)?;
        result.last_position = None;
        Ok(result)
    }

    pub fn push(&mut self, point: Point, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        if point.position == 0
            || point.position > self.plan.source_cut.sequence
            || self.previous_cut.as_ref().is_some_and(|cut| point.position <= cut.sequence)
            || self.last_position.is_some_and(|position| point.position <= position)
        {
            return Err(invalid(
                "supply new rows in strictly increasing source position order after the prior cut and at or before the source cut",
            ));
        }
        self.last_position = Some(point.position);
        if !point.value.is_finite() {
            return Ok(());
        }
        let ordinal = ordinal_of(point.time_ns, self.width_ns);
        let summary = OrderedSummary::new(point);
        self.buckets.entry(ordinal).and_modify(|bucket| bucket.merge(&summary)).or_insert(summary);
        self.compact(check)
    }

    pub fn merge(&mut self, other: Self, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        if self.plan.time != other.plan.time
            || self.plan.value != other.plan.value
            || self.plan.position != other.plan.position
            || self.plan.width_ns != other.plan.width_ns
            || self.plan.max_buckets != other.plan.max_buckets
            || self.plan.source_cut.history != other.plan.source_cut.history
        {
            return Err(invalid("merge only summaries with the same fields, width, bound and history"));
        }
        let width = self.width_ns.max(other.width_ns);
        while self.width_ns < width {
            self.double(check)?;
        }
        let ratio = width / other.width_ns;
        for (ordinal, summary) in other.buckets {
            check()?;
            let ordinal = (u128::from(ordinal) / ratio) as u64;
            self.buckets.entry(ordinal).and_modify(|bucket| bucket.merge(&summary)).or_insert(summary);
        }
        self.plan.source_cut.sequence = self.plan.source_cut.sequence.max(other.plan.source_cut.sequence);
        self.last_position = self.last_position.max(other.last_position);
        self.compact(check)
    }

    fn compact(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        while self.buckets.len() > self.plan.max_buckets.get() {
            self.double(check)?;
        }
        Ok(())
    }

    fn double(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        self.width_ns *= 2;
        let mut merged: BTreeMap<u64, OrderedSummary> = BTreeMap::new();
        for (ordinal, summary) in mem::take(&mut self.buckets) {
            check()?;
            merged.entry(ordinal / 2).and_modify(|bucket| bucket.merge(&summary)).or_insert(summary);
        }
        self.buckets = merged;
        Ok(())
    }

    pub fn continuation(&self) -> Result<Vec<u8>> {
        postcard::to_allocvec(self).map_err(|_| invalid("reduce the bucket bound to encode the continuation"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn ordinal_of(time_ns: i64, width_ns: u128) -> u64 {
    ((i128::from(time_ns) - i128::from(i64::MIN)) as u128 / width_ns) as u64
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{Buckets, OrderedSummary, Point};
    use harmos_domain::Identifier;
    use harmos_domain::model::storage::query::transform::statistics::BucketSummary;
    use harmos_domain::model::storage::{StorageCursor, StoreHistory};
    use std::num::{NonZeroU64, NonZeroUsize};

    fn plan(bound: usize) -> BucketSummary {
        BucketSummary {
            time: "t".into(),
            position: "p".into(),
            value: "v".into(),
            width_ns: NonZeroU64::new(1).unwrap(),
            max_buckets: NonZeroUsize::new(bound).unwrap(),
            source_cut: StorageCursor {
                history: StoreHistory {
                    store: Identifier::new("statistics").unwrap(),
                    epoch: Identifier::new("merge").unwrap(),
                },
                sequence: 60,
            },
            prior: None,
        }
    }
    fn compare(left: &Buckets, right: &Buckets) {
        assert_eq!(left.width_ns, right.width_ns);
        assert_eq!(left.buckets.keys().collect::<Vec<_>>(), right.buckets.keys().collect::<Vec<_>>());
        for (left, right) in left.buckets.values().zip(right.buckets.values()) {
            assert_eq!((left.first, left.last, left.min, left.max), (right.first, right.last, right.min, right.max));
            assert_eq!(left.moments.count(), right.moments.count());
            assert_eq!(left.moments.sum(), right.moments.sum());
            for (left, right) in [
                (left.moments.mean(), right.moments.mean()),
                (left.moments.rms(), right.moments.rms()),
                (left.moments.std(), right.moments.std()),
            ] {
                assert!((left.unwrap() - right.unwrap()).abs() < 1e-11);
            }
        }
    }
    #[test]
    fn bounded_summaries_merge_associatively_with_ordered_points() {
        for bound in [1, 2, 7, 100] {
            let plan = plan(bound);
            let mut whole = Buckets::new(&plan, &mut || Ok(())).unwrap();
            let mut parts = (0..3).map(|_| Buckets::new(&plan, &mut || Ok(())).unwrap()).collect::<Vec<_>>();
            for index in 0..60 {
                let point = Point {
                    time_ns: (index * 37) % 101 - 50,
                    position: index as u64 + 1,
                    value: ((index * 7) % 19) as f64 - 9.0,
                };
                whole.push(point, &mut || Ok(())).unwrap();
                parts[index as usize / 20].push(point, &mut || Ok(())).unwrap();
                assert!(whole.buckets.len() <= bound);
            }
            let mut left = parts[0].clone();
            left.merge(parts[1].clone(), &mut || Ok(())).unwrap();
            left.merge(parts[2].clone(), &mut || Ok(())).unwrap();
            let mut right = parts[1].clone();
            right.merge(parts[2].clone(), &mut || Ok(())).unwrap();
            let mut grouped = parts[0].clone();
            grouped.merge(right, &mut || Ok(())).unwrap();
            compare(&left, &whole);
            compare(&grouped, &whole);
            let mut reverse = parts[2].clone();
            reverse.merge(parts[0].clone(), &mut || Ok(())).unwrap();
            reverse.merge(parts[1].clone(), &mut || Ok(())).unwrap();
            compare(&reverse, &whole);
        }
    }
    #[test]
    fn bucket_width_covers_both_timestamp_extremes_at_a_single_bucket_bound() {
        let mut summary = Buckets::new(&plan(1), &mut || Ok(())).unwrap();
        for (position, time_ns) in [i64::MIN, i64::MAX].into_iter().enumerate() {
            summary
                .push(
                    Point {
                        time_ns,
                        position: position as u64 + 1,
                        value: position as f64,
                    },
                    &mut || Ok(()),
                )
                .unwrap();
        }
        assert_eq!(summary.buckets.len(), 1);
        let bucket = summary.buckets.values().next().unwrap();
        assert_eq!(bucket.first.time_ns, i64::MIN);
        assert_eq!(bucket.last.time_ns, i64::MAX);
        assert_eq!(bucket.moments.count(), 2);
    }
    #[test]
    fn ordered_summary_preserves_points_moments_and_extrema_ties() {
        let points = [(9, 1, 5.0), (3, 2, -2.0), (6, 3, 8.0), (9, 4, 5.0), (1, 5, -2.0)].map(|(time_ns, position, value)| Point {
            time_ns,
            position,
            value,
        });
        let mut summary = OrderedSummary::new(points[0]);
        for point in &points[1..] {
            summary.merge(&OrderedSummary::new(*point));
        }
        assert_eq!(summary.first, points[4]);
        assert_eq!(summary.last, points[3]);
        assert_eq!(summary.min, points[4]);
        assert_eq!(summary.max, points[2]);
        assert_eq!(summary.moments.count(), 5);
        assert_eq!(summary.moments.sum(), 14.0);
        assert_eq!(summary.moments.squares(), 122.0);
        for (actual, expected) in
            [(summary.moments.mean(), 2.8), (summary.moments.rms(), 24.4_f64.sqrt()), (summary.moments.std(), 16.56_f64.sqrt())]
        {
            assert!((actual.unwrap() - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn continuation_refuses_changed_contracts_and_invalid_positions() {
        let mut original = Buckets::new(&plan(2), &mut || Ok(())).unwrap();
        original.push(Point { time_ns: 0, position: 10, value: 2.0 }, &mut || Ok(())).unwrap();
        let bytes = original.continuation().unwrap();
        let mut continuation = plan(2);
        continuation.prior = Some(bytes.clone());
        assert!(Buckets::new(&continuation, &mut || Ok(())).is_ok());
        for change in 0..4 {
            let mut changed = continuation.clone();
            match change {
                0 => changed.max_buckets = NonZeroUsize::new(3).unwrap(),
                1 => changed.width_ns = NonZeroU64::new(2).unwrap(),
                2 => changed.source_cut.sequence -= 1,
                _ => changed.source_cut.history.epoch = Identifier::new("other").unwrap(),
            }
            assert!(Buckets::new(&changed, &mut || Ok(())).is_err());
        }
        continuation.prior = Some(bytes[..bytes.len() / 2].to_vec());
        assert!(Buckets::new(&continuation, &mut || Ok(())).is_err());
        assert!(original.push(Point { time_ns: 0, position: 10, value: 3.0 }, &mut || Ok(())).is_err());
        assert!(original.push(Point { time_ns: 0, position: 61, value: 3.0 }, &mut || Ok(())).is_err());
    }
}
