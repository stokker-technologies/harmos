/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod read;

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
#[harmos::catalog]
pub enum Implemented {
    Read(read::Read),
    Heard(crate::listeners::recalibrated::Heard),
}

#[harmos::catalog]
pub enum Operations {
    #[nested]
    Implemented(Implemented),
    Start(crate::observers::controls::Start),
    Stop(crate::observers::controls::Stop),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
