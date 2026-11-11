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

#[derive(harmos::Message, Clone, Debug, Default)]
struct Leaf {
    value: u64,
}

// Repeated messages already live on the heap inside their vector.
#[derive(harmos::Message, Clone, Debug, Default)]
struct Holder {
    rows: Vec<Box<Leaf>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
