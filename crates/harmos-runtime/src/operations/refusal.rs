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
use std::time::Duration;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("operation {operation} attempt {attempt} exceeded its {} s deadline (configured by {configured_by})", .timeout.as_secs_f64())]
    TimedOut { operation: String, attempt: usize, timeout: Duration, configured_by: &'static str },

    #[error("operation {operation} attempt {attempt} was cancelled")]
    Cancelled { operation: String, attempt: usize },

    #[error("the operation runtime became unavailable")]
    Unavailable,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
