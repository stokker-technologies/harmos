/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Packaging Build, Bundle And Inspection Failures

use crate::bundle;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("build cancelled after retaining and settling its child processes")]
    Cancelled,

    #[error(transparent)]
    Watch(#[from] notify::Error),

    #[error(transparent)]
    Bundle(#[from] bundle::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Declaration(#[from] harmos_domain::inventory::DeclarationError),

    #[error("invalid packaging settings: {0}")]
    Settings(String),

    #[error("Cargo build failed: {0}")]
    Cargo(String),

    #[error("guest componentization failed: {0}")]
    Component(String),

    #[error("built artifact changed after stamping: {0}")]
    ArtifactChanged(std::path::PathBuf),

    #[error("generated artifact collision: {0}")]
    Collision(String),
}
