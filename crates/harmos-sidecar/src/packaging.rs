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
use std::path::{Path, PathBuf};

// Third Party Imports

// Local Imports
use crate::inventory::{Declaration, Facility};
use harmos_contract::grpc::Artifacts;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Read(#[from] std::io::Error),
    #[error(transparent)]
    Declaration(#[from] crate::inventory::DeclarationError),
    #[error(transparent)]
    Artifacts(#[from] harmos_contract::grpc::ExportError),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn export(artifact: &Path, manifest: &Path, name: &str) -> Result<PathBuf, ExportError> {
    let bytes = std::fs::read(artifact)?;
    let declaration = Declaration::inspect(&bytes, Facility::Sidecar)?;
    let artifacts = Artifacts::new(&declaration.contracts.provides)?;
    Ok(artifacts.export_cargo(manifest, name)?)
}

/* === Private Functions === */

/* === Tests === */
