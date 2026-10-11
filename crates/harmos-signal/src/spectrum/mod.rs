/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[cfg(feature = "fft")]
mod bounded;
mod decibel;
#[cfg(feature = "fft")]
mod fourier;
mod harmonic;
mod indicator;
mod taper;

/* === Re-Exports === */
#[cfg(feature = "fft")]
pub use bounded::fft_in_place;
pub use decibel::{Decibel, Scale};
#[cfg(feature = "fft")]
pub use fourier::{Estimate, Welch, reconstruct, transform};
pub use harmonic::{fundamental, harmonics, peaks, thd};
pub use indicator::{band_energy, band_rms, centroid, occupied_bandwidth, papr, rolloff};
pub use taper::{Band, Taper};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    pub step: f64,
    pub real: Vec<f64>,
    pub imag: Vec<f64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Spectrum {
    #[must_use]
    pub fn rate(&self) -> f64 {
        match self.real.len() {
            0 | 1 => 0.0,
            bins => self.step * ((bins - 1) * 2) as f64,
        }
    }
}

/* === Public Functions === */
#[must_use]
pub fn magnitude(real: &[f64], imag: &[f64]) -> Vec<f64> {
    real.iter()
        .zip(imag)
        .map(|(real, imag)| real.hypot(*imag))
        .collect()
}

#[must_use]
pub fn phase(real: &[f64], imag: &[f64]) -> Vec<f64> {
    real.iter()
        .zip(imag)
        .map(|(real, imag)| imag.atan2(*real))
        .collect()
}

#[must_use]
pub fn bins(step: f64, count: usize) -> Vec<f64> {
    (0..count).map(|index| index as f64 * step).collect()
}

/* === Private Functions === */

/* === Tests === */
