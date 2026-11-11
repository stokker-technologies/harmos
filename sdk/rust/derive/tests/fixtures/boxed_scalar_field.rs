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

// A box owns a nested message; a boxed scalar has no message to own.
#[derive(harmos::Message, Clone, Debug)]
struct Holder {
    row: Box<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
