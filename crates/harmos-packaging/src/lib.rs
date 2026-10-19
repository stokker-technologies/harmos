/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reusable Cargo Build, Inert Inspection And Guest Componentization.

/* === Modules === */

mod build;
pub mod bundle;
#[cfg(feature = "guest")]
mod component;
pub mod config;
mod dev;
mod metadata;
mod process;
#[cfg(feature = "generate-proto")]
pub mod proto;
#[cfg(feature = "generate-python")]
pub mod python;

/* === Re-Exports === */

pub use build::{Artifact, Build, PackageOutput, Schema, build, build_with_cancellation, package, package_with_cancellation};
#[cfg(feature = "guest")]
pub use component::componentize;
pub use dev::{DevEvent, dev};
pub use harmos_domain::inventory::{Declaration, DeclarationError, Facility};
pub use process::BuildCancellation;

/* === Imports === */

// Standard Library Imports
use std::path::Path;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug, serde::Serialize)]
pub struct Inspection {
    pub id: String,
    pub version: String,
    pub description: String,
    pub config: String,
    pub operations: Vec<String>,
    pub sha256: String,
    pub facility: &'static str,
}

/* === Enums === */

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

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn inspect(path: &Path) -> Result<Inspection, Error> {
    let bytes = std::fs::read(path)?;
    let guest = bytes.starts_with(b"\0asm");
    let facility = if guest { Facility::Guest } else { Facility::Sidecar };
    let declaration = Declaration::inspect(&bytes, facility)?;
    Ok(Inspection {
        id: declaration.id,
        version: declaration.version.to_string(),
        description: declaration.description,
        config: declaration.config,
        operations: declaration.operations.iter().map(|operation| operation.id.to_string()).collect(),
        sha256: digest(&bytes),
        facility: if guest { "guest" } else { "sidecar" },
    })
}

/// Lowercase hexadecimal SHA-256 of `bytes`, the one digest form every stamp and marker uses.
pub fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(hash.len() * 2);
    for byte in hash {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

/* === Private Functions === */

/* === Tests === */
