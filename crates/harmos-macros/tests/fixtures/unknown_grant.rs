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

struct Program;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos::guest(harmos_path = ::harmos, id = "invalid_requirement", requires(harmos::builtins::streams::Publish: invoke("ledger.deposit")))]
impl harmos::guest::Plugin for Program {}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
