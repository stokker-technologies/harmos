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
mod spectrum;
mod waveform;

/* === Re-Exports === */
pub use aggregates::Aggregates;
pub use error::{Error, Result};
pub use spectrum::{
    Band, Decibel, Scale, Spectrum, Taper, band_energy, band_rms, bins, centroid, fundamental,
    harmonics, magnitude, occupied_bandwidth, papr, peaks, phase, rolloff, thd,
};
#[cfg(feature = "fft")]
pub use spectrum::{Estimate, Welch, fft_in_place, reconstruct, transform};
pub use waveform::{
    Edge, Series, Summary, above_floor, crossing, decimate, derivative, edges, extrema, integral,
    lttb, lttb_log, nearest, resample, resample_count, resample_uniform, value_at,
};

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
