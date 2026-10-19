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
use std::ops::Range;
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

#[must_use]
pub fn band_energy(frequency: &[f64], magnitude: &[f64], low: f64, high: f64) -> f64 {
    let band = band(frequency, low, high);
    if band.is_empty() {
        return 0.0;
    }

    let step = step_of(frequency, &band);
    magnitude[band].iter().map(|value| value * value * step).sum()
}

#[must_use]
pub fn band_rms(frequency: &[f64], magnitude: &[f64], low: f64, high: f64) -> f64 {
    let bandwidth = high - low;
    if bandwidth <= 0.0 {
        return 0.0;
    }

    (band_energy(frequency, magnitude, low, high) / bandwidth).sqrt()
}

#[must_use]
pub fn centroid(frequency: &[f64], magnitude: &[f64], low: f64, high: f64) -> Option<f64> {
    let band = band(frequency, low, high);
    if band.is_empty() {
        return None;
    }

    let (weighted, total) = frequency[band.clone()].iter().zip(&magnitude[band]).fold((0.0, 0.0), |(weighted, total), (at, value)| {
        let power = value * value;
        (weighted + at * power, total + power)
    });

    (total > 0.0).then_some(weighted / total)
}

#[must_use]
pub fn papr(frequency: &[f64], magnitude: &[f64], low: f64, high: f64) -> Option<f64> {
    let band = &magnitude[band(frequency, low, high)];
    if band.is_empty() {
        return None;
    }

    let peak = band.iter().copied().fold(0.0_f64, f64::max);
    let mean = band.iter().sum::<f64>() / band.len() as f64;

    (mean > 0.0).then(|| (peak * peak) / (mean * mean))
}

#[must_use]
pub fn rolloff(frequency: &[f64], magnitude: &[f64], low: f64, high: f64, fraction: f64) -> Option<f64> {
    let band = band(frequency, low, high);
    if band.is_empty() {
        return None;
    }

    let step = step_of(frequency, &band);
    let total: f64 = magnitude[band.clone()].iter().map(|value| value * value * step).sum();
    if total <= 0.0 {
        return None;
    }

    let target = total * fraction.clamp(0.0, 1.0);
    let mut accumulated = 0.0;

    for (at, value) in frequency[band.clone()].iter().zip(&magnitude[band.clone()]) {
        accumulated += value * value * step;
        if accumulated >= target {
            return Some(*at);
        }
    }

    frequency.get(band.end - 1).copied()
}

#[must_use]
pub fn occupied_bandwidth(frequency: &[f64], magnitude: &[f64], low: f64, high: f64, fraction: f64) -> Option<(f64, f64, f64)> {
    let band = band(frequency, low, high);
    if band.is_empty() {
        return None;
    }

    let step = step_of(frequency, &band);
    let energies: Vec<f64> = magnitude[band.clone()].iter().map(|value| value * value * step).collect();
    let total: f64 = energies.iter().sum();
    if total <= 0.0 {
        return None;
    }

    let target = total * fraction.clamp(0.0, 1.0);
    // Prefix sums, so the energy of any window is one subtraction.
    let mut prefix = vec![0.0; energies.len() + 1];
    for (index, energy) in energies.iter().enumerate() {
        prefix[index + 1] = prefix[index] + energy;
    }

    let (mut lower, mut upper, mut width) = (0, energies.len() - 1, energies.len());
    let mut left = 0;

    for right in 0..energies.len() {
        // Shrink from the left while the window still clears the target, so
        // what is checked below is the narrowest window ending here.
        while left < right && prefix[right + 1] - prefix[left + 1] >= target {
            left += 1;
        }

        if prefix[right + 1] - prefix[left] >= target && right - left + 1 < width {
            width = right - left + 1;
            lower = left;
            upper = right;
        }
    }

    let lower = frequency[band.start + lower];
    let upper = frequency[band.start + upper];

    Some((lower, upper, upper - lower + step))
}

/* === Private Functions === */

fn band(frequency: &[f64], low: f64, high: f64) -> Range<usize> {
    let start = frequency.partition_point(|at| *at < low);
    let end = frequency.partition_point(|at| *at <= high);

    start..end.max(start)
}

fn step_of(frequency: &[f64], band: &Range<usize>) -> f64 {
    match band.len() {
        0 => 1.0,
        1 => match (band.start.checked_sub(1), frequency.get(band.end)) {
            (Some(before), _) => frequency[band.start] - frequency[before],
            (None, Some(after)) => after - frequency[band.start],
            (None, None) => 1.0,
        },
        length => (frequency[band.end - 1] - frequency[band.start]) / (length - 1) as f64,
    }
}

/* === Tests === */
