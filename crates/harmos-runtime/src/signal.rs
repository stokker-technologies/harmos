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
// The signal kernels whole, beside the trait that answers the contract
// aggregate vocabulary from them.
pub use harmos_signal::*;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::{Aggregate, aggregate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
// One member of the contract vocabulary, answered from a `Series` (every
// member) or a `Summary` (the mergeable members; median and percentiles are
// `None`, since no constant-size state can answer them).
pub trait Aggregates {
    fn aggregate(&self, kind: Aggregate) -> Option<f64>;
}

/* === Implementations === */
impl Aggregates for Series<'_> {
    fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_series(self, kind)
    }
}

impl Aggregates for Summary {
    fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_summary(self, kind)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
