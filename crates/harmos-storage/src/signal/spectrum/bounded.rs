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

struct Slices<'a> {
    real: &'a mut [f64],
    imag: &'a mut [f64],
}

/* === Enums === */

/* === Traits === */

pub(super) trait Buffer {
    fn len(&self) -> usize;
    fn get(&mut self, index: usize) -> Result<(f64, f64)>;
    fn set(&mut self, index: usize, value: (f64, f64)) -> Result<()>;
}

/* === Implementations === */

impl Buffer for Slices<'_> {
    fn len(&self) -> usize {
        self.real.len()
    }
    fn get(&mut self, index: usize) -> Result<(f64, f64)> {
        Ok((self.real[index], self.imag[index]))
    }
    fn set(&mut self, index: usize, value: (f64, f64)) -> Result<()> {
        self.real[index] = value.0;
        self.imag[index] = value.1;
        Ok(())
    }
}

/* === Public Functions === */

pub fn fft_in_place(real: &mut [f64], imag: &mut [f64], check: impl FnMut() -> bool) -> Result<()> {
    if real.len() != imag.len() {
        return Err(Error::InvalidInput {
            reason: "FFT requires equal finite power-of-two buffers",
        });
    }
    let mut buffer = Slices { real, imag };
    transform(&mut buffer, check)
}

pub(super) fn transform(buffer: &mut impl Buffer, mut check: impl FnMut() -> bool) -> Result<()> {
    let length = buffer.len();
    if !length.is_power_of_two() {
        return Err(Error::InvalidInput {
            reason: "FFT requires equal finite power-of-two buffers",
        });
    }
    validate(buffer, &mut check, "FFT requires equal finite power-of-two buffers")?;
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
            let left = buffer.get(index)?;
            let right = buffer.get(reversed)?;
            buffer.set(index, right)?;
            buffer.set(reversed, left)?;
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
                let (left_real, left_imag) = buffer.get(left)?;
                let (right_real, right_imag) = buffer.get(right)?;
                let r = right_real * phase_real - right_imag * phase_imag;
                let i = right_real * phase_imag + right_imag * phase_real;
                buffer.set(right, (left_real - r, left_imag - i))?;
                buffer.set(left, (left_real + r, left_imag + i))?;
                (phase_real, phase_imag) = (phase_real * cos - phase_imag * sin, phase_real * sin + phase_imag * cos);
            }
        }
        if width == length {
            break;
        }
        width *= 2;
    }
    validate(buffer, &mut check, "FFT exceeds finite range")
}

/* === Private Functions === */

fn validate(buffer: &mut impl Buffer, check: &mut impl FnMut() -> bool, reason: &'static str) -> Result<()> {
    for index in 0..buffer.len() {
        if !check() {
            return Err(Error::Interrupted);
        }
        let (real, imag) = buffer.get(index)?;
        if !real.is_finite() || !imag.is_finite() {
            return Err(Error::InvalidInput { reason });
        }
    }
    Ok(())
}

/* === Tests === */
