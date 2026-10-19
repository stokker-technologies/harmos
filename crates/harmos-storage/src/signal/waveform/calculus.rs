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
pub fn derivative(x: &[f64], y: &[f64]) -> Vec<f64> {
    let length = x.len().min(y.len());
    if length < 2 {
        return Vec::new();
    }

    let mut slope = Vec::with_capacity(length);
    slope.push((y[1] - y[0]) / (x[1] - x[0]));
    slope.extend((1..length - 1).map(|index| (y[index + 1] - y[index - 1]) / (x[index + 1] - x[index - 1])));
    slope.push((y[length - 1] - y[length - 2]) / (x[length - 1] - x[length - 2]));

    slope
}

#[must_use]
pub fn integral(x: &[f64], y: &[f64]) -> Vec<f64> {
    let length = x.len().min(y.len());
    if length == 0 {
        return Vec::new();
    }

    let mut accumulated = 0.0;
    let mut area = Vec::with_capacity(length);
    area.push(0.0);

    for index in 1..length {
        accumulated += (y[index] + y[index - 1]) / 2.0 * (x[index] - x[index - 1]);
        area.push(accumulated);
    }

    area
}

/* === Private Functions === */

/* === Tests === */
