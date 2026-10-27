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
pub fn lttb(x: &[f64], y: &[f64], count: usize) -> Vec<usize> {
    let length = x.len().min(y.len());
    if length <= count || count < 3 {
        return (0..length).collect();
    }

    let mut kept = Vec::with_capacity(count);
    kept.push(0);

    let bucket = (length - 2) as f64 / (count - 2) as f64;
    let mut previous = 0;

    for index in 0..count - 2 {
        let start = 1 + (index as f64 * bucket).floor() as usize;
        let end = (1 + ((index + 1) as f64 * bucket).floor() as usize).min(length - 1);
        let ahead = if index < count - 3 { (1 + ((index + 2) as f64 * bucket).floor() as usize).min(length) } else { length };

        let (mean_x, mean_y) = mean_of(x, y, end..ahead).unwrap_or((x[length - 1], y[length - 1]));

        let picked = (start..end)
            .max_by(|left, right| {
                area(x[previous], y[previous], x[*left], y[*left], mean_x, mean_y).total_cmp(&area(
                    x[previous],
                    y[previous],
                    x[*right],
                    y[*right],
                    mean_x,
                    mean_y,
                ))
            })
            .unwrap_or(start);

        kept.push(picked);
        previous = picked;
    }

    kept.push(length - 1);
    kept
}

#[must_use]
pub fn lttb_log(x: &[f64], y: &[f64], count: usize) -> Vec<usize> {
    let length = x.len().min(y.len());
    if length <= count || count < 3 {
        return (0..length).collect();
    }

    let (first, last) = (positive(x[0]), x[length - 1]);
    if !last.is_finite() || last <= first {
        return lttb(x, y, count);
    }

    let (low, span) = (first.ln(), last.ln() - first.ln());
    if span <= 0.0 {
        return lttb(x, y, count);
    }

    let buckets = count - 2;
    let edge = |bucket: usize| (low + (bucket as f64 / buckets as f64) * span).exp();

    let mut kept = Vec::with_capacity(count);
    kept.push(0);
    let mut previous = 0;

    for bucket in 0..buckets {
        let start = x[..length].partition_point(|at| *at < edge(bucket)).min(length - 1);
        let end = x[..length].partition_point(|at| *at < edge(bucket + 1)).clamp(start + 1, length);
        let ahead = if bucket < buckets - 1 {
            x[..length].partition_point(|at| *at < edge(bucket + 2).min(last)).clamp(end, length)
        } else {
            length
        };

        let (mean_x, mean_y) = mean_of(x, y, end..ahead).unwrap_or((x[length - 1], y[length - 1]));
        // The triangle is measured where the eye sees it: along ln(x).
        let (previous_x, mean_x) = (positive(x[previous]).ln(), positive(mean_x).ln());

        let picked = (start..end)
            .max_by(|left, right| {
                let of = |bin: usize| area(previous_x, y[previous], positive(x[bin]).ln(), y[bin], mean_x, mean_y);
                of(*left).total_cmp(&of(*right))
            })
            .unwrap_or(start);

        kept.push(picked);
        previous = picked;
    }

    kept.push(length - 1);
    kept.dedup();
    kept
}

#[must_use]
pub fn decimate(length: usize, count: usize) -> Vec<usize> {
    decimate_indices(length, count).collect()
}

/// Iterates The Same Uniform Indices As [`decimate`] Without Allocating.
pub fn decimate_indices(length: usize, count: usize) -> impl ExactSizeIterator<Item = usize> + DoubleEndedIterator + Clone {
    if length == 0 || count == 0 {
        return (0..0).step_by(1);
    }

    let stride = (length as f64 / count as f64).ceil() as usize;
    (0..length).step_by(stride.max(1))
}

#[must_use]
pub fn extrema(values: &[f64], buckets: usize) -> Vec<usize> {
    if values.is_empty() || buckets == 0 {
        return Vec::new();
    }

    let span = ((values.len() as f64 / buckets as f64).ceil() as usize).max(1);
    let mut kept = Vec::with_capacity(buckets * 2);

    for (bucket, samples) in values.chunks(span).enumerate() {
        // Strict comparisons on purpose: a bucket whose samples never differ
        // leaves both ends on the first one, and contributes it alone.
        let (mut lowest, mut highest) = (0, 0);
        for (index, value) in samples.iter().enumerate() {
            if *value < samples[lowest] {
                lowest = index;
            }
            if *value > samples[highest] {
                highest = index;
            }
        }

        let base = bucket * span;
        kept.push(base + lowest.min(highest));
        if lowest != highest {
            kept.push(base + lowest.max(highest));
        }
    }

    kept
}

#[must_use]
pub fn above_floor(values: &[f64], db_below_peak: f64) -> Vec<usize> {
    let peak = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if peak.is_nan() || peak <= 0.0 {
        return Vec::new();
    }

    let floor = peak * 10.0_f64.powf(-db_below_peak / 20.0);
    values.iter().enumerate().filter(|(_, value)| **value >= floor).map(|(index, _)| index).collect()
}

/* === Private Functions === */

fn positive(at: f64) -> f64 {
    at.max(f64::MIN_POSITIVE)
}

fn area(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x0 - x2) * (y1 - y0) - (x0 - x1) * (y2 - y0)).abs()
}

fn mean_of(x: &[f64], y: &[f64], range: std::ops::Range<usize>) -> Option<(f64, f64)> {
    let count = range.len();
    if count == 0 {
        return None;
    }

    let sum_x: f64 = x[range.clone()].iter().sum();
    let sum_y: f64 = y[range].iter().sum();

    Some((sum_x / count as f64, sum_y / count as f64))
}

/* === Tests === */

#[test]
fn decimation_indices_preserve_uniform_stride_and_empty_cases() {
    for (length, count, expected) in
        [(0, 4, vec![]), (8, 0, vec![]), (1, 1, vec![0]), (5, 8, vec![0, 1, 2, 3, 4]), (8, 3, vec![0, 3, 6]), (9, 3, vec![0, 3, 6])]
    {
        let mut indices = decimate_indices(length, count);
        assert_eq!(indices.len(), expected.len());
        assert_eq!(indices.clone().collect::<Vec<_>>(), expected);
        assert_eq!(decimate(length, count), expected);
        if let Some(last) = expected.last() {
            assert_eq!(indices.next_back(), Some(*last));
            assert_eq!(indices.len(), expected.len() - 1);
        }
    }
}
