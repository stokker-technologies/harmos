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
// A contribution argument lists the types that are it. One with no types
// contributes nothing, and the attribute simply omits the argument instead.
pub struct Bridge;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::plugin(description = "Empty contribution fixture.",
    id = "bridge",
    provides = (), requires = (), observers(),
    restart = "never"
)]
impl harmos::Plugin for Bridge {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
