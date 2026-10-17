/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export The Producer's Explicit Schematic Operation Catalog.

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contracts = harmos::operations::Contracts::of::<schematic_domain::operations::Native>()?;
    let artifacts = harmos::grpc::Artifacts::new(&contracts)?;
    artifacts.export_build("operations")?;
    Ok(())
}

/* === Tests === */
