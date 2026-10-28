/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod descriptor;
mod export;
pub mod reflection;

pub use descriptor::{DescriptorError, descriptor};
pub use export::{Artifacts, ExportError};

pub fn generate<'a>(
    catalog: impl IntoIterator<Item = &'a harmos_domain::operations::Contract>, output: &std::path::Path,
) -> Result<std::path::PathBuf, ExportError> {
    let artifacts = Artifacts::new(catalog)?;
    artifacts.write(output)
}
