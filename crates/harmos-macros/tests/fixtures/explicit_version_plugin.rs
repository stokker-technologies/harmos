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

// Executable versions come from Cargo; an authored second version is refused.
pub struct Settings;

pub struct Bridge;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos::plugin(description = "A bridge.", id = "bridge", requires = (), version = "1.0.0")]
impl harmos::Plugin for Bridge {}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
