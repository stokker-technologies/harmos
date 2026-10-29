/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod aggregates;
mod error;
#[cfg(feature = "fft")]
mod spectrum;
mod waveform;

pub use aggregates::Aggregates;
pub use error::{Error, Result};
#[cfg(feature = "fft")]
pub use spectrum::{FftBuffer, fft_in_place};
pub use waveform::{Series, Summary, decimate, decimate_indices, extrema, lttb, lttb_log};
