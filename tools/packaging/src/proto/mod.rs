/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod export;

/* === Re-Exports === */

pub use export::{Artifacts, ExportError};
pub use harmos_runtime::grpc::projection::{DescriptorError, descriptor};
pub use harmos_runtime::grpc::schema as reflection;

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
    catalog: impl IntoIterator<Item = &'a harmos_operations::operations::Contract>, output: &std::path::Path,
) -> Result<std::path::PathBuf, ExportError> {
    let artifacts = Artifacts::new(catalog)?;
    artifacts.write(output)
}

/* === Private Functions === */

/* === Tests === */
