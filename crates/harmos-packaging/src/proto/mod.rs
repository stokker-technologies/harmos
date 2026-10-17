/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod descriptor;
mod export;
pub mod reflection;

/* === Re-Exports === */
pub use descriptor::{DescriptorError, descriptor};
pub use export::{Artifacts, ExportError};

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
pub fn generate<'a>(
    catalog: impl IntoIterator<Item = &'a harmos_domain::operations::Contract>,
    output: &std::path::Path,
) -> Result<std::path::PathBuf, ExportError> {
    let artifacts = Artifacts::new(catalog)?;
    artifacts.write(output)
}


/* === Private Functions === */

/* === Tests === */
