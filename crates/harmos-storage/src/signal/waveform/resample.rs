/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[must_use]
pub fn value_at(x: &[f64], y: &[f64], at: f64) -> f64 {
    let length = x.len().min(y.len());
    match length {
        0 => return 0.0,
        1 => return y[0],
        _ => {}
    }

    if at <= x[0] {
        return y[0];
    }
    if at >= x[length - 1] {
        return y[length - 1];
    }

    let above = x[..length].partition_point(|sample| *sample < at).clamp(1, length - 1);
    let below = above - 1;
    let span = x[above] - x[below];

    if span.abs() < f64::EPSILON {
        return (y[below] + y[above]) / 2.0;
    }

    y[below] + (at - x[below]) / span * (y[above] - y[below])
}

#[must_use]
pub fn resample(x: &[f64], y: &[f64], at: &[f64]) -> Vec<f64> {
    at.iter().map(|at| value_at(x, y, *at)).collect()
}

#[must_use]
pub fn resample_uniform(x: &[f64], y: &[f64], step: f64) -> Vec<f64> {
    let length = x.len().min(y.len());
    if length == 0 || step <= 0.0 {
        return Vec::new();
    }

    let span = x[length - 1] - x[0];
    let count = (span / step).floor() as usize + 1;

    (0..count).map(|index| value_at(x, y, x[0] + index as f64 * step)).collect()
}

#[must_use]
pub fn resample_count(values: &[f64], count: usize) -> Vec<f64> {
    if values.is_empty() || count == 0 {
        return Vec::new();
    }
    if values.len() == 1 || count == 1 {
        return vec![values[0]; count];
    }

    let step = (values.len() - 1) as f64 / (count - 1) as f64;
    let axis: Vec<f64> = (0..values.len()).map(|index| index as f64).collect();

    (0..count).map(|index| value_at(&axis, values, index as f64 * step)).collect()
}

#[must_use]
pub fn nearest(x: &[f64], at: f64) -> Option<usize> {
    if x.is_empty() {
        return None;
    }

    let above = x.partition_point(|sample| *sample < at);
    if above == 0 {
        return Some(0);
    }
    if above >= x.len() {
        return Some(x.len() - 1);
    }

    let below = above - 1;
    Some(if at - x[below] <= x[above] - at { below } else { above })
}
