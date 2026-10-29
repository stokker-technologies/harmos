/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use crate::signal::{Series, Summary};

use harmos_domain::Aggregate;

// Answers one member of the contract vocabulary from the signal math. The
// kernels live in the sibling `signal` module; these functions connect them to
// core reductions and for `signal::Aggregates` alike. Empty input answers
// `Count` and `Sum` with zero and every other member with `None`.
pub fn of_series(series: &Series<'_>, kind: Aggregate) -> Option<f64> {
    match kind {
        Aggregate::Median => series.median(),
        Aggregate::Percentile(rank) => series.percentile(f64::from(rank)),
        mergeable => of_summary(&series.summary(), mergeable),
    }
}

pub fn of_summary(summary: &Summary, kind: Aggregate) -> Option<f64> {
    match kind {
        Aggregate::Count => Some(summary.count() as f64),
        Aggregate::Sum => Some(summary.sum()),
        Aggregate::Mean => summary.mean(),
        Aggregate::Min => summary.min(),
        Aggregate::Max => summary.max(),
        Aggregate::Rms => summary.rms(),
        Aggregate::Std => summary.std(),
        // A summary keeps constant state so it can merge, and an order
        // statistic is not a function of any constant state: the median of a
        // union depends on where every value falls, not on per-part counts
        // and moments. Refusing beats an estimate that reads exact.
        Aggregate::Median | Aggregate::Percentile(_) => None,
    }
}
