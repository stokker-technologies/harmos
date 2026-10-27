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

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct ListDirectoriesRequest {}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Directories {
    pub directories: Vec<String>,
}

#[harmos::operation(
    id = "schematic_list_directories",
    request = ListDirectoriesRequest, infer_request, response = Directories,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    grpc(package = "schematic.v1", service = "Schematic", method = "ListDirectories"),
)]
pub struct ListDirectories {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for ListDirectories {
    type Request = ListDirectoriesRequest;
    type Response = Directories;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(_: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self {})
    }

    async fn run(
        &mut self, _: harmos::OperationContext, _: &mut Self::Request, _: &mut (),
    ) -> Result<harmos::Response<Directories>, Self::Error> {
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
