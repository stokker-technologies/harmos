/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Collect and Package Repository Fixtures

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

// Third Party Imports
use harmos_packaging::{
    Build,
    config::{BuildBackend, BuildProfile, BuildSettings},
};
use serde::Deserialize;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Component {
        module: PathBuf,
        output: PathBuf,
    },
    Collect {
        manifest: PathBuf,
        package: String,
        backend: BuildBackend,
        profile: BuildProfile,
        target: String,
        messages: PathBuf,
        metadata: PathBuf,
        receipt: PathBuf,
    },
    Package {
        build: PathBuf,
        from: PathBuf,
        to: PathBuf,
        output: PathBuf,
    },
    Python {
        export: PathBuf,
        output: PathBuf,
        protoc: PathBuf,
    },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read The Repository-Owned Request.
    let request: Request = serde_json::from_reader(std::io::stdin().lock())?;
    let output = match request {
        Request::Component { module, output } => {
            let bytes = std::fs::read(module)?;
            let component = harmos_packaging::componentize(&bytes).map_err(std::io::Error::other)?;
            std::fs::write(&output, component)?;
            serde_json::json!({"component": output})
        }
        Request::Collect {
            manifest,
            package,
            backend,
            profile,
            target,
            messages,
            metadata,
            receipt,
        } => {
            let settings = BuildSettings {
                manifest,
                packages: vec![package],
                backend,
                profile,
                target: Some(target),
                ..BuildSettings::default()
            };
            let build =
                Build::from_cargo_outputs(BufReader::new(File::open(messages)?), File::open(metadata)?, File::open(receipt)?, &settings)?;
            serde_json::to_value(build)?
        }
        Request::Package { build, from, to, output } => {
            let mut build: Build = serde_json::from_reader(File::open(build)?)?;
            build.relocate(&from, &to)?;
            let packages = harmos_packaging::package(&build, &output)?;
            serde_json::to_value(packages)?
        }
        Request::Python { export, output, protoc } => {
            let artifacts = harmos_packaging::proto::Artifacts::read(&export)?;
            let contracts = artifacts.operation_contracts()?;
            let settings = harmos_packaging::python::Settings {
                distribution: "schematic-client".into(),
                module: "schematic_client".into(),
                version: "0.0.0".parse()?,
                description: "Schematic Host Client".into(),
                license: "Apache-2.0".into(),
                operations: Vec::new(),
            };
            let catalog = harmos_packaging::python::Catalog {
                contracts: &contracts,
                settings: &settings,
                protoc: &protoc,
            };
            let generated = harmos_packaging::python::generate(catalog, &output)?;
            serde_json::to_value(generated)?
        }
    };
    serde_json::to_writer(std::io::stdout().lock(), &output)?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
