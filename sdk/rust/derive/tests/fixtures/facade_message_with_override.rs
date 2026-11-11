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

#[derive(facade::Row)]
#[harmos_path(::facade::__harmos)]
#[derive(Debug, Default)]
#[derive(facade::Serialize, facade::Deserialize)]
#[serde(crate = "facade")]
#[derive(Clone, PartialEq)]
struct Reading {
    value: u64,

    bytes: Vec<u8>,
}

#[derive(facade::Message, Debug, Default)]
#[harmos_path(::facade::__harmos)]
#[derive(Clone, PartialEq)]
struct Sample {
    reading: Option<Reading>,
    channels: Vec<u32>,
}

#[derive(facade::Row)]
#[harmos_path(::facade::__harmos)]
#[serde(crate = "facade::__harmos::__private::serde")]
#[derive(Clone, Debug, Default, PartialEq, facade::__harmos::__private::serde::Serialize, facade::__harmos::__private::serde::Deserialize)]
struct Portable {
    #[row(label = "Measured Value", unit = "V")]
    value: f64,
}

#[derive(facade::Message, Clone, Debug, Default)]
#[harmos_path(::facade::__harmos)]
#[cfg(any())]
struct Excluded {
    value: u32,
}

#[derive(facade::Message, Clone, Default)]
#[harmos_path(::facade::__harmos)]
struct CustomDebug {
    value: u32,
}

/* === Enums === */

#[derive(facade::Message, Clone, Debug)]
#[harmos_path(::facade::__harmos)]
enum Outcome {
    Pending,
    Ready { sample: Sample },
    Failed(String),
}

impl Default for Outcome { fn default() -> Self { Self::Pending } }


#[derive(facade::Message, Clone, Debug)]
#[harmos_path(::facade::__harmos)]
#[cfg(any())]
enum ExcludedEnum {
    Pending,
    Ready(u32),
}
#[cfg(any())]
impl Default for ExcludedEnum { fn default() -> Self { Self::Pending } }


/* === Traits === */

/* === Implementations === */

impl ::facade::__harmos::streams::StreamRow for Reading {
    const ENTRY: ::facade::__harmos::StreamEntry<Self> = ::facade::__harmos::StreamEntry::new("reading", 8).label("Reading");
}

impl ::facade::__harmos::streams::StreamRow for Portable {
    const ENTRY: ::facade::__harmos::StreamEntry<Self> = ::facade::__harmos::StreamEntry::new("portable", 4).label("Portable");
}

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
}

/* === Tests === */
