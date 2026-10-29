/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export The Producer's Explicit Schematic Operation Catalog.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contracts = harmos_domain::operations::Contracts::of::<schematic_operations::Native>()?;
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?).join("grpc/operations");
    harmos_packaging::proto::generate(&contracts, &output)?;
    Ok(())
}
