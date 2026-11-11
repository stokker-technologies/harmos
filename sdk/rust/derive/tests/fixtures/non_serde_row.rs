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

#[derive(harmos::Row)]
#[derive(Clone)]
struct Pulse;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::streams::StreamRow for Pulse {
    const ENTRY: harmos::StreamEntry<Self> = harmos::StreamEntry::new("pulse", 8).label("Pulse");
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
