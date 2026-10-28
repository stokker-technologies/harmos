/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

use super::{Band, Spectrum, Taper};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Welch {
    pub resolution: f64,
    pub overlap: f64,
    pub taper: Taper,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Estimate {
    pub step: f64,
    pub magnitude: Vec<f64>,
}

impl Default for Welch {
    fn default() -> Self {
        Self {
            resolution: 100.0,
            overlap: 0.5,
            taper: Taper::Hann,
        }
    }
}

impl Welch {
    #[must_use]
    pub fn new(resolution: f64) -> Self {
        Self { resolution, ..Self::default() }
    }

    #[must_use]
    pub fn estimate(&self, samples: &[f64], rate: f64) -> Estimate {
        let total = samples.len();
        if total == 0 || rate <= 0.0 {
            return Estimate { step: 0.0, magnitude: Vec::new() };
        }

        let segment = ((rate / self.resolution).round() as usize).clamp(64, total);
        let hop = (((segment as f64) * (1.0 - self.overlap.clamp(0.0, 0.99))).round() as usize).max(1);
        let segments = if total <= segment { 1 } else { (total - segment) / hop + 1 };

        if segments <= 1 || segment >= total {
            let single = transform(samples, rate, self.taper);
            return Estimate {
                step: single.step,
                magnitude: super::magnitude(&single.real, &single.imag),
            };
        }

        let bins = segment / 2 + 1;
        let coefficients = self.taper.coefficients(segment);
        let gain: f64 = coefficients.iter().sum();

        let mut power = vec![0.0; bins];
        let mut planner = FftPlanner::new();
        let plan = planner.plan_fft_forward(segment);

        for index in 0..segments {
            let start = index * hop;
            let Some(window) = samples.get(start..start + segment) else {
                break;
            };

            let mut buffer: Vec<Complex<f64>> =
                window.iter().zip(&coefficients).map(|(sample, coefficient)| Complex { re: sample * coefficient, im: 0.0 }).collect();

            plan.process(&mut buffer);

            for (accumulated, bin) in power.iter_mut().zip(buffer.iter().take(bins)) {
                *accumulated += bin.norm_sqr();
            }
        }

        // Summed over K segments the power of a tone is K·A²·(Σw)²/4, so
        // dividing by K·(Σw)² and doubling the root recovers A.
        let normalizer = segments as f64 * gain.powi(2);
        let mut magnitude: Vec<f64> = power.iter().map(|power| (power / normalizer).sqrt() * 2.0).collect();

        magnitude[0] /= 2.0;
        if segment.is_multiple_of(2) && bins > 1 {
            magnitude[bins - 1] /= 2.0;
        }

        Estimate { step: rate / segment as f64, magnitude }
    }
}

#[must_use]
#[tracing::instrument(level = "trace", skip_all, fields(samples = samples.len(), rate))]
pub fn transform(samples: &[f64], rate: f64, taper: Taper) -> Spectrum {
    let length = samples.len();
    if length == 0 || rate <= 0.0 {
        return Spectrum {
            step: 0.0,
            real: Vec::new(),
            imag: Vec::new(),
        };
    }

    let bins = length / 2 + 1;
    let coefficients = taper.coefficients(length);
    // The coherent gain, summed rather than averaged: dividing the transform by
    // it is what makes one taper's amplitude the same as another's.
    let gain: f64 = coefficients.iter().sum();

    let mut buffer: Vec<Complex<f64>> =
        samples.iter().zip(&coefficients).map(|(sample, coefficient)| Complex { re: sample * coefficient, im: 0.0 }).collect();

    FftPlanner::new().plan_fft_forward(length).process(&mut buffer);

    let mut real: Vec<f64> = buffer.iter().take(bins).map(|bin| bin.re / gain * 2.0).collect();
    let mut imag: Vec<f64> = buffer.iter().take(bins).map(|bin| bin.im / gain * 2.0).collect();

    // A single-sided spectrum doubles every bin that has a negative-frequency
    // twin. DC has none, and neither does Nyquist when the length is even.
    real[0] /= 2.0;
    imag[0] /= 2.0;
    if length.is_multiple_of(2) && bins > 1 {
        real[bins - 1] /= 2.0;
        imag[bins - 1] /= 2.0;
    }

    Spectrum { step: rate / length as f64, real, imag }
}

#[must_use]
pub fn reconstruct(spectrum: &Spectrum, band: Option<Band>) -> Vec<f64> {
    let bins = spectrum.real.len();
    if bins < 2 {
        return Vec::new();
    }

    let length = (bins - 1) * 2;
    let mut buffer: Vec<Complex<f64>> = vec![Complex::default(); length];

    for (index, (real, imag)) in spectrum.real.iter().zip(&spectrum.imag).enumerate() {
        let coefficient = band.map_or(1.0, |band| band.coefficient(index as f64 * spectrum.step));
        // Undo exactly what the forward transform did: DC and Nyquist were
        // never doubled, so they are scaled by the whole length and the rest by
        // half of it.
        let scale = if index == 0 || index == bins - 1 { length as f64 } else { length as f64 / 2.0 };

        buffer[index] = Complex {
            re: real * coefficient * scale,
            im: imag * coefficient * scale,
        };
    }

    for index in 1..bins - 1 {
        buffer[length - index] = buffer[index].conj();
    }

    FftPlanner::new().plan_fft_inverse(length).process(&mut buffer);

    buffer.iter().map(|sample| sample.re / length as f64).collect()
}
