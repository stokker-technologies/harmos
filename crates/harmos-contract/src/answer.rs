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

/* === Traits === */
pub trait Answer {
    type Ok;
    type Err;
}

/* === Implementations === */
impl<T, E> Answer for Result<T, E> {
    type Ok = T;
    type Err = E;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
