/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Adapt Explicit Producer Selection To The Shared Python Generator.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{io::Read as _, path::PathBuf};
// Third Party Imports
// Local Imports
use harmos_packaging::{
    proto::Artifacts,
    python::{self, Settings},
};

/* === Constants === */
const MAX_BUILD_BYTES: u64 = 16 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Args)]
pub struct Arguments {
    #[arg(help = "An Exact Cargo OUT_DIR Catalog Export; No Directory Discovery.")]
    #[arg(
        long,
        conflicts_with = "build_json",
        required_unless_present = "build_json"
    )]
    export: Option<PathBuf>,
    #[arg(help = "JSON Returned By A Successful Cargo Harmos Build.")]
    #[arg(long, conflicts_with = "export", requires_all = ["producer", "catalog"])]
    build_json: Option<PathBuf>,
    #[arg(long, requires = "build_json")]
    producer: Option<String>,
    #[arg(long, requires = "build_json")]
    catalog: Option<String>,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    distribution: String,
    #[arg(long)]
    module: String,
    #[arg(long)]
    package_version: semver::Version,
    #[arg(long)]
    description: String,
    #[arg(long)]
    license: String,
    #[arg(
        help = "Select Operation Identities; The Default Requires Every Contract To Be Protobuf-Bound."
    )]
    #[arg(long)]
    operation: Vec<String>,
    #[arg(long, default_value = "protoc")]
    protoc: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(arguments: Arguments) -> Result<(), crate::error::Error> {
    let export = selected_export(&arguments)?;
    let artifacts = Artifacts::read(&export).map_err(python::Error::from)?;
    let settings = Settings {
        distribution: arguments.distribution,
        module: arguments.module,
        version: arguments.package_version,
        description: arguments.description,
        license: arguments.license,
        operations: arguments.operation,
    };
    let contracts = artifacts
        .operation_contracts()
        .map_err(python::Error::from)?;
    let catalog = python::Catalog {
        contracts: &contracts,
        settings: &settings,
        protoc: &arguments.protoc,
    };
    let output = python::generate(catalog, &arguments.output)?;
    println!("{}", output.display());
    Ok(())
}

/* === Private Functions === */
fn selected_export(arguments: &Arguments) -> Result<PathBuf, python::Error> {
    if let Some(path) = &arguments.export {
        return Ok(path.clone());
    }
    let path = arguments
        .build_json
        .as_ref()
        .ok_or(python::Error::Settings(
            "an exact export or build JSON is required",
        ))?;
    let producer = arguments
        .producer
        .as_deref()
        .ok_or(python::Error::Settings("producer is required"))?;
    let catalog = arguments
        .catalog
        .as_deref()
        .ok_or(python::Error::Settings("catalog is required"))?;
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_BUILD_BYTES {
        return Err(python::Error::Limit);
    }
    let mut bytes = Vec::new();
    file.take(MAX_BUILD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BUILD_BYTES {
        return Err(python::Error::Limit);
    }
    let build: harmos_packaging::Build = serde_json::from_slice(&bytes)?;
    let mut entries = build
        .schemas
        .into_iter()
        .filter(|entry| entry.producer == producer && entry.catalog == catalog);
    let entry = entries
        .next()
        .ok_or_else(|| python::Error::Selection(format!("{producer}/{catalog}")))?;
    if entries.next().is_some() {
        return Err(python::Error::Collision(format!("{producer}/{catalog}")));
    }
    Ok(entry.directory)
}

/* === Tests === */
