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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
#[must_use]
pub fn peaks(magnitude: &[f64]) -> Vec<usize> {
    let mut found: Vec<usize> = Vec::new();

    if magnitude.len() >= 2 {
        if magnitude[0] > magnitude[1] {
            found.push(0);
        }
        found.extend(
            magnitude
                .windows(3)
                .enumerate()
                .filter(|(_, window)| window[1] > window[0] && window[1] > window[2])
                .map(|(index, _)| index + 1),
        );
        let last = magnitude.len() - 1;
        if magnitude[last] > magnitude[last - 1] {
            found.push(last);
        }
    }

    found.sort_by(|left, right| magnitude[*right].total_cmp(&magnitude[*left]));
    found
}

#[must_use]
pub fn harmonics(magnitude: &[f64], fraction: f64) -> Vec<usize> {
    let strongest = magnitude.iter().copied().fold(0.0_f64, f64::max);
    let floor = fraction * strongest;

    let mut found: Vec<usize> = peaks(magnitude)
        .into_iter()
        .filter(|bin| magnitude[*bin] >= floor)
        .collect();

    found.sort_unstable();
    found
}

#[must_use]
pub fn fundamental(frequency: &[f64], magnitude: &[f64]) -> Option<usize> {
    let guard = step_of(frequency) * 0.5;

    peaks(magnitude)
        .into_iter()
        .find(|bin| frequency.get(*bin).is_some_and(|at| *at > guard))
}

#[must_use]
pub fn thd(frequency: &[f64], magnitude: &[f64]) -> Option<f64> {
    let bin = fundamental(frequency, magnitude)?;
    let strongest = magnitude[bin];
    if strongest < f64::EPSILON {
        return None;
    }

    let tolerance = step_of(frequency) * 0.5;
    let distortion: f64 = magnitude
        .iter()
        .zip(frequency)
        .filter(|(_, at)| **at > tolerance && (**at - frequency[bin]).abs() > tolerance)
        .map(|(value, _)| value * value)
        .sum();

    Some(distortion.sqrt() / strongest * 100.0)
}

/* === Private Functions === */
fn step_of(frequency: &[f64]) -> f64 {
    match frequency {
        [first, second, ..] => second - first,
        _ => 1.0,
    }
}

/* === Tests === */
