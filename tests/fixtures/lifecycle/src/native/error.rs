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
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("worker payload exceeds its byte budget ({actual} > {limit})")]
    PayloadTooLarge { actual: usize, limit: usize },
    #[error("worker operations remain unsettled ({finalized}/{constructed} finalized)")]
    Unsettled { constructed: u64, finalized: u64 },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
