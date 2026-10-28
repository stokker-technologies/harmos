/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("signal workspace I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid signal input: {reason}")]
    InvalidInput { reason: &'static str },

    #[error("signal computation interrupted")]
    Interrupted,
}
