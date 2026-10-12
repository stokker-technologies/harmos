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
// Identity Is Authored On The Unified Implementation, Where Its Contract Is Known
#[harmos::plugin(description = "A bridge.", id = "bridge", provides = (), requires = ())]
pub struct Bridge;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::Plugin for Bridge {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
