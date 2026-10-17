/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Render Inert Artifact Inspection In Human Or JSON Form.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Args)]
pub struct Arguments {
    artifact: std::path::PathBuf,
    #[arg(long)]
    json: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(arguments: Arguments) -> Result<(), harmos_packaging::Error> {
    let inspected = harmos_packaging::inspect(&arguments.artifact)?;
    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&inspected)?);
    } else {
        println!("{}@{} ({})\n{}", inspected.id, inspected.version, inspected.facility, inspected.sha256);
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
