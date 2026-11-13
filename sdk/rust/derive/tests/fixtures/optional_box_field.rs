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

// An optional message stays `Option<T>`; boxing inside the option is refused.
#[derive(harmos::Message, Clone, Debug, Default)]
struct Holder {
    row: Option<Box<Leaf>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
