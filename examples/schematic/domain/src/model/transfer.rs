/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Bounded Transfer Data.

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

#[harmos_domain::message]
#[derive(Clone)]
pub struct Record {
    pub sequence: u64,
    pub payload: Vec<u8>,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Sample {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub value: f64,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Batch {
    pub records: Vec<Record>,
    pub samples: Vec<Sample>,
    pub bytes: Vec<u8>,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Receipt {
    pub records: u64,
    pub samples: u64,
    pub bytes: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
