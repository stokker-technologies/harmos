/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use harmos_signal::{Series, Summary};

// Local Imports
use harmos_domain::Aggregate;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
// Answers one member of the contract vocabulary from the signal math. The
// kernels live in `harmos-signal`, which names nothing of Harmos so it lifts
// out unchanged; these two functions are where the vocabulary meets them, for
// core reductions and for `signal::Aggregates` alike. Empty input answers
// `Count` and `Sum` with zero and every other member with `None`.
pub(crate) fn of_series(series: &Series<'_>, kind: Aggregate) -> Option<f64> {
    match kind {
        Aggregate::Median => series.median(),
        Aggregate::Percentile(rank) => series.percentile(f64::from(rank)),
        mergeable => of_summary(&series.summary(), mergeable),
    }
}

pub(crate) fn of_summary(summary: &Summary, kind: Aggregate) -> Option<f64> {
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

/* === Private Functions === */

/* === Tests === */
