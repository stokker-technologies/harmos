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
#[harmos::stream(
    description = "A portable reading",
    id = "reading",
    label = "Reading",
    window = 7
)]
pub struct Reading {
    #[row(label = "Measured Value", role = value, unit = "V", tags("electrical"), attributes(channel = 1))]
    pub value: u32,
}

#[harmos::stream(id = "ungranted", window = 11)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Ungranted {
    pub value: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
