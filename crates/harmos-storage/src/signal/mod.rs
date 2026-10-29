/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod aggregates;
mod error;
#[cfg(feature = "fft")]
mod spectrum;
mod waveform;

/* === Re-Exports === */

pub use aggregates::Aggregates;
pub use error::{Error, Result};
#[cfg(feature = "fft")]
pub use spectrum::{FftBuffer, fft_in_place};
pub use waveform::{Series, Summary, decimate, decimate_indices, extrema, lttb, lttb_log};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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
