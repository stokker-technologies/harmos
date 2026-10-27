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
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::transaction(id = "sheet.place",  inverse = Remove)]
#[derive(Deserialize, Serialize)]
struct Place;

#[harmos::transaction(id = "sheet.remove",  inverse = Self)]
#[derive(Deserialize, Serialize)]
struct Remove;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
