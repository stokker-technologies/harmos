/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub name: String,
    pub version: semver::Version,
    pub description: String,

    #[serde(default)]
    pub package: PackageMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMetadata {
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub identity: Identity,
    pub files: Vec<File>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: String,
    pub size: u64,
    pub sha256: String,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub executable: Option<Executable>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub facility: Facility,
    pub target: String,
    pub registrations: Vec<harmos_domain::Registration>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub archive_bytes: usize,
    pub manifest_bytes: usize,
    pub file_bytes: usize,
    pub total_file_bytes: usize,
    pub files: usize,
    pub registrations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Facility {
    Sidecar,
    Guest,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            archive_bytes: 272 * 1024 * 1024,
            manifest_bytes: 16 * 1024 * 1024,
            file_bytes: 128 * 1024 * 1024,
            total_file_bytes: 256 * 1024 * 1024,
            files: 4096,
            registrations: 1024,
        }
    }
}
