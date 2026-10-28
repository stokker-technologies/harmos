/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin bundle errors

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid plugin bundle: {0}")]
    Invalid(String),

    #[error("plugin bundle exceeds {0}")]
    Limit(&'static str),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
