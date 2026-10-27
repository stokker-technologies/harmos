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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub at: f64,
    pub rising: bool,
}

/* === Enums === */

#[derive(Clone, Copy)]
enum Level {
    Beneath,
    Between,
    Above,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[must_use]
pub fn edges(x: &[f64], y: &[f64], low: f64, high: f64) -> Vec<Edge> {
    let length = x.len().min(y.len());
    if length < 2 {
        return Vec::new();
    }

    let lowest = y[..length].iter().copied().fold(f64::INFINITY, f64::min);
    let highest = y[..length].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = highest - lowest;
    if !range.is_finite() || range < f64::EPSILON {
        return Vec::new();
    }

    let (low, high) = (lowest + range * low, lowest + range * high);
    let mut state = level(y[0], low, high);
    let mut found = Vec::new();

    for index in 1..length {
        let (previous, current) = (y[index - 1], y[index]);

        match state {
            Level::Beneath if current > high => {
                found.push(Edge {
                    at: crossed(x, index, high, previous, current),
                    rising: true,
                });
                state = Level::Above;
            }
            Level::Above if current < low => {
                found.push(Edge {
                    at: crossed(x, index, low, previous, current),
                    rising: false,
                });
                state = Level::Beneath;
            }
            Level::Between => state = level(current, low, high),
            Level::Beneath | Level::Above => {}
        }
    }

    found
}

#[must_use]
pub fn crossing(x: &[f64], y: &[f64], target: f64) -> Option<f64> {
    let length = x.len().min(y.len());

    (1..length).find_map(|index| {
        let (previous, current) = (y[index - 1], y[index]);
        let straddles = (previous <= target && target <= current) || (current <= target && target <= previous);

        straddles.then(|| crossed(x, index, target, previous, current))
    })
}

/* === Private Functions === */

fn level(value: f64, low: f64, high: f64) -> Level {
    if value > high {
        Level::Above
    } else if value < low {
        Level::Beneath
    } else {
        Level::Between
    }
}

fn crossed(x: &[f64], index: usize, target: f64, previous: f64, current: f64) -> f64 {
    let (before, after) = (x[index - 1], x[index]);
    let rise = current - previous;

    if rise.abs() < f64::EPSILON {
        return before;
    }

    before + (target - previous) / rise * (after - before)
}

/* === Tests === */
