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
use std::path::PathBuf;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1).map(PathBuf::from);
    let input = arguments
        .next()
        .ok_or("usage: harmos-component <core.wasm> <guest.wasm>")?;
    let output = arguments
        .next()
        .ok_or("usage: harmos-component <core.wasm> <guest.wasm>")?;
    if arguments.next().is_some() {
        return Err("usage: harmos-component <core.wasm> <guest.wasm>".into());
    }

    let component = harmos_guest::componentize(&std::fs::read(&input)?)
        .map_err(|message| format!("{}: {message}", input.display()))?;
    let declaration = harmos_guest::inventory::Declaration::inspect(
        &component,
        harmos_guest::inventory::Facility::Guest,
    )?;
    let name = output
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("output must have a UTF-8 artifact name")?;
    let artifacts = harmos_contract::grpc::Artifacts::new(&declaration.contracts.provides)?;
    let directory = artifacts.export_cargo(std::path::Path::new("Cargo.toml"), name)?;
    std::fs::write(&output, component)?;
    println!(
        "built {} {}@{} (protobuf: {})",
        output.display(),
        declaration.id,
        declaration.version,
        directory.display()
    );
    Ok(())
}

/* === Tests === */
