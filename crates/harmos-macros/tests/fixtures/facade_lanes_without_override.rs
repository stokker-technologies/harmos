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

/* === Enums === */
// The regression half of facade_lanes_with_override.rs.
#[facade::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Workflow,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    let _ = Lanes::catalog();
}

/* === Tests === */
