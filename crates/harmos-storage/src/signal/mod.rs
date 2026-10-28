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
mod spectrum;
mod waveform;

pub use aggregates::Aggregates;
pub use error::{Error, Result};
pub use spectrum::{
    Band, Decibel, Scale, Spectrum, Taper, band_energy, band_rms, bins, centroid, fundamental, harmonics, magnitude, occupied_bandwidth,
    papr, peaks, phase, rolloff, thd,
};
#[cfg(feature = "fft")]
pub use spectrum::{Estimate, FftBuffer, Welch, fft_in_place, reconstruct, transform};
pub use waveform::{
    Edge, Series, Summary, above_floor, crossing, decimate, decimate_indices, derivative, edges, extrema, integral, lttb, lttb_log,
    nearest, resample, resample_count, resample_uniform, value_at,
};
