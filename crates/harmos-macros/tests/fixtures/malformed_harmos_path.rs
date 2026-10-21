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

// A facade crate names its own path with `harmos_path = …`, which every
// path-emitting attribute accepts and parses as a crate path. A value that is
// not one — a string, here — is refused where it is written.
#[harmos::stream(id = "pulse", window = 8, harmos_path = "not a path")]
struct Pulse;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
