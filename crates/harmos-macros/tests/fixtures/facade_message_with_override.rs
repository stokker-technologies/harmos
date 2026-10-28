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
extern crate facade as serde;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[facade::stream(id = "batched", window = 8, batch(samples = 3, gaps = "report"), harmos_path = ::facade::__harmos)]
#[derive(facade::Serialize, facade::Deserialize)]
#[serde(crate = "facade")]
#[facade::message(harmos_path = ::facade::__harmos)]
#[derive(Clone, PartialEq)]
struct Reading {
    #[harmos(measure)]
    value: u64,

    bytes: Vec<u8>,
}

#[facade::message(harmos_path = ::facade::__harmos)]
#[derive(Clone, PartialEq)]
struct Sample {
    reading: Option<Reading>,
    channels: Vec<u32>,
}

#[facade::stream(id = "portable", label = "Portable", window = 4, harmos_path = ::facade::__harmos)]
struct Portable {
    #[row(label = "Measured Value", unit = "V")]
    value: f64,
}

#[facade::message(harmos_path = ::facade::__harmos)]
#[cfg(any())]
struct Excluded {
    value: u32,
}

#[facade::message(harmos_path = ::facade::__harmos, skip_debug)]
struct CustomDebug {
    value: u32,
}

/* === Enums === */

#[facade::message(harmos_path = ::facade::__harmos)]
enum Outcome {
    Pending,
    Ready { sample: Sample },
    Failed(String),
}

#[facade::message(harmos_path = ::facade::__harmos)]
#[cfg(any())]
enum ExcludedEnum {
    Pending,
    Ready(u32),
}

/* === Traits === */

/* === Implementations === */

impl std::fmt::Debug for CustomDebug {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CustomDebug").field("value", &self.value).finish()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let _ = Outcome::default();
    let _ = Portable::default();
    let _ = Sample::default();
    assert_eq!(<Reading as facade::__harmos::streams::Batched>::SAMPLES, 3);
}

/* === Tests === */
