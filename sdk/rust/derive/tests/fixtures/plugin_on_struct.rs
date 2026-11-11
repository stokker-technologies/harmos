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

// Identity Is Authored on the Unified Implementation, Where Its Contract Is Known
#[harmos::guest(harmos_path = ::harmos, description = "A bridge.", id = "bridge", requires = ())]
pub struct Bridge;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::guest::Plugin for Bridge {}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
