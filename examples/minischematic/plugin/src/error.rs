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
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Publish(#[from] harmos::guest::PublishError),
    #[error(transparent)]
    Host(#[from] harmos::guest::OperationRefusal),
    #[error("the keyed retry did not return the original receipt")]
    RetryMismatch,

    #[error(transparent)]
    Encode(#[from] serde_json::Error),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
