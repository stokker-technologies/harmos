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
use schematic_contract::native::list_directories::{Request, Response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct ListDirectories;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_sidecar::operation(declaration = schematic_contract::native::ListDirectories)]
impl ListDirectories {
    async fn initialize(_: Arc<()>, _: &Supervisor) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(&mut self, _: &Supervisor, _: &mut Request) -> Result<Response, Error> {
        // Native Filesystem Access: The Guest Has No Such Capability.
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
        Ok(Response { directories })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
