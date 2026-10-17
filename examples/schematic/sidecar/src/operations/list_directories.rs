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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Error;
use harmos_sidecar::Supervisor;
use schematic_domain::operations::ListDirectories;
use schematic_domain::operations::directories::list_directories::Directories;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Listing;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_sidecar::operation]
impl Listing {
    async fn initialize(_: Arc<crate::Resources>, _: &Supervisor) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(&mut self, _: &Supervisor, _: &mut ListDirectories) -> Result<Directories, Error> {
        // Native Filesystem Access: The Guest Has No Such Access.
        let entries = std::fs::read_dir(".")?;
        let mut directories = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(Error::DirectoryName)?;
                directories.push(name);
            }
        }
        directories.sort();
        Ok(Directories { directories })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
