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

#[facade::lanes(harmos_path = ::facade::__harmos)]
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
    let _ = <Lanes as facade::__harmos::operations::Lane>::catalog();
}

/* === Tests === */
