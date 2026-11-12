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

pub(super) const ROWS: usize = 2_000_000;
const DAYS_NS: i64 = 3 * 24 * 60 * 60 * 1_000_000_000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
pub(super) struct Sample {
    pub time_ns: i64,
    pub plot: u32,
    pub value: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn sample(index: usize) -> Sample {
    let logical = if index % 4096 == 4095 { index - 2048 } else { index };
    Sample {
        time_ns: (logical as i128 * i128::from(DAYS_NS) / ROWS as i128) as i64,
        plot: (logical % 24) as u32,
        value: ((logical as u64 * 7919) % 65521) as f64,
    }
}

/* === Private Functions === */

/* === Tests === */
