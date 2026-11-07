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

// Third Party Imports

// Local Imports

/* === Constants === */

const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    id = "schematic.list.directories",
    response = Directories,
    policy = POLICY,
    visibility = Public,
    grpc(package = "schematic.v1", service = "Schematic", method = "ListDirectories"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ListDirectories {}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Directories {
    pub directories: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for ListDirectories {
    type Error = crate::Error;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<Directories>, Self::Error> {
        let entries = std::fs::read_dir(".")?;
        let mut directories = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().into_string().map_err(crate::Error::DirectoryName)?;
                directories.push(name);
            }
        }
        directories.sort();
        Ok(Directories { directories }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
