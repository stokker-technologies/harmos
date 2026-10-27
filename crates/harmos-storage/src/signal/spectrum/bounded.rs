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

// Local Imports
use crate::signal::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn fft_in_place(real: &mut [f64], imag: &mut [f64], mut check: impl FnMut() -> bool) -> Result<()> {
    let length = real.len();
    if !length.is_power_of_two() || length != imag.len() || real.iter().chain(imag.iter()).any(|value| !value.is_finite()) {
        return Err(Error::InvalidInput {
            reason: "FFT requires equal finite power-of-two buffers",
        });
    }
    let mut reversed = 0;
    for index in 1..length {
        if !check() {
            return Err(Error::Interrupted);
        }
        let mut bit = length >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            real.swap(index, reversed);
            imag.swap(index, reversed);
        }
    }
    let mut width = 2usize;
    while width <= length {
        let angle = -std::f64::consts::TAU / width as f64;
        let (sin, cos) = angle.sin_cos();
        for start in (0..length).step_by(width) {
            let (mut phase_real, mut phase_imag) = (1.0, 0.0);
            for offset in 0..width / 2 {
                if !check() {
                    return Err(Error::Interrupted);
                }
                let left = start + offset;
                let right = left + width / 2;
                let r = real[right] * phase_real - imag[right] * phase_imag;
                let i = real[right] * phase_imag + imag[right] * phase_real;
                real[right] = real[left] - r;
                imag[right] = imag[left] - i;
                real[left] += r;
                imag[left] += i;
                (phase_real, phase_imag) = (phase_real * cos - phase_imag * sin, phase_real * sin + phase_imag * cos);
            }
        }
        if width == length {
            break;
        }
        width *= 2;
    }
    if real.iter().chain(imag.iter()).any(|value| !value.is_finite()) {
        return Err(Error::InvalidInput { reason: "FFT exceeds finite range" });
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
