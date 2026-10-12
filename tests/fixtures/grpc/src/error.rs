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
use thiserror::Error;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Runtime(#[from] harmos::Error),
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
