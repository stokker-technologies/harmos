/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reusable Cargo Build, Inert Inspection.

/* === Modules === */

mod build;
pub mod bundle;
pub mod config;
mod dev;
mod error;
mod metadata;
mod process;
#[cfg(feature = "generate-proto")]
pub mod proto;
#[cfg(feature = "generate-python")]
pub mod python;

/* === Re-Exports === */

pub use build::{Artifact, Build, PackageOutput, Schema, build, build_with_cancellation, package, package_with_cancellation};
pub use dev::{DevEvent, dev};
pub use error::Error;
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

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn inspect(path: &Path) -> Result<Inspection, Error> {
    let bytes = std::fs::read(path)?;
    let facility = Facility::Sidecar;
    let declaration = Declaration::inspect(&bytes, facility)?;

    let mut operations = Vec::with_capacity(declaration.operations.len());
    for operation in &declaration.operations {
        operations.push(operation.id.to_string());
    }

    Ok(Inspection {
        id: declaration.id,
        version: declaration.version.to_string(),
        description: declaration.description,
        config: declaration.config,
        operations,
        sha256: digest(&bytes),
        facility: "sidecar",
    })
}

pub fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(hash.len() * 2);
    for byte in hash {
        let high = char::from(HEX[usize::from(byte >> 4)]);
        let low = char::from(HEX[usize::from(byte & 0x0f)]);
        encoded.push(high);
        encoded.push(low);
    }
    encoded
}

/* === Private Functions === */

/* === Tests === */
