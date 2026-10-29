/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate A Pure Python Producer Package From Verified Contract Artifacts.

mod package;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use semver::Version;

use crate::proto::{Artifacts, ExportError};

pub struct Catalog<'a> {
    pub contracts: &'a [harmos_domain::operations::Contract],
    pub settings: &'a Settings,
    pub protoc: &'a Path,
}

// Producer-Owned Distribution Metadata; No Transport Or Runtime Registration.
#[derive(Clone, Debug)]
pub struct Settings {
    pub distribution: String,
    pub module: String,
    pub version: Version,
    pub description: String,
    pub license: String,
    // An Empty Selection Exports Every Contract; Every Selected Contract Must Have gRPC.
    pub operations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Python binding setting: {0}")]
    Settings(&'static str),

    #[error("unsupported Python binding contract: {0}")]
    Unsupported(String),

    #[error("Python binding name collision: {0}")]
    Collision(String),

    #[error("unknown selected operation: {0}")]
    Selection(String),

    #[error("Python binding output already exists: {0}")]
    Exists(PathBuf),

    #[error("protobuf compiler failed: {0}")]
    Compiler(String),

    #[error("generated Python package exceeds its bounded output limit")]
    Limit,

    #[error("failed to remove incomplete output after {cause}: {source}")]
    Cleanup { cause: String, source: std::io::Error },

    #[error(transparent)]
    Export(#[from] ExportError),

    #[error(transparent)]
    Descriptor(#[from] prost_reflect::DescriptorError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Settings {
    fn validate(&self) -> Result<(), Error> {
        if !identifier(&self.module) || self.module.starts_with('_') {
            return Err(Error::Settings("module must be one public Python identifier"));
        }
        let name = self.distribution.as_bytes();
        if name.is_empty()
            || name.len() > 128
            || !name.first().is_some_and(u8::is_ascii_alphanumeric)
            || !name.last().is_some_and(u8::is_ascii_alphanumeric)
            || !name.iter().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(Error::Settings("distribution name"));
        }
        if self.description.len() > 4096 || self.license.is_empty() || self.license.len() > 256 {
            return Err(Error::Settings("description or license"));
        }
        python_version(&self.version)?;
        Ok(())
    }
}

// Export Standard Protobuf Classes, Stubs And Typed Descriptors Into A New Package Root.
//
// `protoc` Is An Explicit Maintainer Tool. Neither Importing Nor Building The Result
// Requires Rust, Harmos, A Compiler, Or The Producer's Source Tree.
pub fn generate(catalog: Catalog<'_>, output: &Path) -> Result<PathBuf, Error> {
    let settings = catalog.settings;
    let protoc = catalog.protoc;
    settings.validate()?;
    if output.symlink_metadata().is_ok() {
        return Err(Error::Exists(output.to_owned()));
    }
    let contracts = catalog.contracts;
    let mut requested = BTreeSet::new();
    for id in &settings.operations {
        if !requested.insert(id.as_str()) {
            return Err(Error::Collision(id.clone()));
        }
    }
    let all = requested.is_empty();
    let mut selected = Vec::new();
    for contract in contracts {
        if all || requested.remove(contract.operation().id.as_str()) {
            if contract.grpc().is_none() {
                return Err(Error::Unsupported(format!("{} has no protobuf/gRPC binding", contract.operation().id)));
            }
            selected.push(contract.clone());
        }
    }
    if let Some(id) = requested.first() {
        return Err(Error::Selection((*id).into()));
    }
    if selected.is_empty() {
        return Err(Error::Settings("empty operation export"));
    }
    let artifacts = Artifacts::new(&selected)?;
    package::export(&artifacts, &selected, settings, output, protoc)
}

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 128
        && bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(
            value,
            "False"
                | "None"
                | "True"
                | "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "try"
                | "while"
                | "with"
                | "yield"
        )
}

fn python_version(version: &Version) -> Result<String, Error> {
    let mut result = format!("{}.{}.{}", version.major, version.minor, version.patch);
    if !version.pre.is_empty() {
        let mut parts = version.pre.as_str().split('.');
        let kind = match parts.next() {
            Some("alpha") => "a",
            Some("beta") => "b",
            Some("rc") => "rc",
            Some("dev") => ".dev",
            _ => return Err(Error::Settings("unsupported Python prerelease version")),
        };
        let number = parts.next().unwrap_or("0");
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) || parts.next().is_some() {
            return Err(Error::Settings("unsupported Python prerelease number"));
        }
        result.push_str(kind);
        result.push_str(number);
    }
    if !version.build.is_empty() {
        result.push('+');
        result.push_str(&version.build.as_str().replace('-', ".").to_ascii_lowercase());
    }
    Ok(result)
}
