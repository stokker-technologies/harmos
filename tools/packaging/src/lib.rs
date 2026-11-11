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

#[cfg(feature = "build-tools")]
mod build;
pub mod bundle;
#[cfg(feature = "guest")]
mod component;
#[cfg(feature = "build-tools")]
pub mod config;
#[cfg(feature = "build-tools")]
mod dev;
mod error;
#[cfg(feature = "build-tools")]
mod metadata;
#[cfg(feature = "sidecar")]
mod native;
#[cfg(feature = "build-tools")]
mod process;
#[cfg(feature = "generate-proto")]
pub mod proto;
#[cfg(feature = "generate-python")]
pub mod python;

/* === Re-Exports === */

#[cfg(feature = "build-tools")]
pub use build::{Artifact, Build, PackageOutput, Schema, build, build_with_cancellation, package, package_with_cancellation};
#[cfg(feature = "guest")]
pub use component::componentize;
#[cfg(feature = "build-tools")]
pub use dev::{DevEvent, dev};
pub use error::Error;
pub use harmos_operations::inventory::{Declaration, DeclarationError, Facility};
pub use harmos_runtime::bundle::digest;
#[cfg(feature = "build-tools")]
pub use process::BuildCancellation;

/* === Imports === */

// Standard Library Imports
use std::path::Path;

// Third Party Imports

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
    let guest = bytes.starts_with(b"\0asm");
    if !guest {
        #[cfg(feature = "sidecar")]
        {
            let description = native::describe(path, &BuildCancellation::default())?;
            return Ok(Inspection {
                id: description.id,
                version: description.version,
                description: String::new(),
                config: String::new(),
                operations: description.catalog.into_iter().flat_map(|catalog| catalog.operations).map(|operation| operation.id).collect(),
                sha256: digest(&bytes),
                facility: "sidecar",
            });
        }
        #[cfg(not(feature = "sidecar"))]
        return Err(Error::Settings("native inspection requires the sidecar feature".into()));
    }
    let facility = if guest { Facility::Guest } else { Facility::Sidecar };
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
        facility: if guest { "guest" } else { "sidecar" },
    })
}

/* === Private Functions === */

/* === Tests === */
