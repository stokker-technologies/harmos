/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod archive;
mod metadata;

/* === Re-Exports === */

pub use harmos_runtime::bundle::{Bundle, Error, Executable, Facility, File, Identity, Limits, Manifest, PackageMetadata};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Source<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
    pub executable: Option<Executable>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn build(identity: Identity, sources: &[Source<'_>], limits: Limits) -> Result<Bundle, Error> {
    build_with_settings(identity, sources, limits, &harmos_domain::config::SchemaSettings::default())
}

pub fn build_with_settings(
    identity: Identity, sources: &[Source<'_>], limits: Limits, settings: &harmos_domain::config::SchemaSettings,
) -> Result<Bundle, Error> {
    if sources.len() > limits.files {
        return Err(Error::Limit("file count"));
    }
    let mut files = Vec::with_capacity(sources.len());
    let mut contents = BTreeMap::new();
    let mut total = 0usize;
    let mut metadata_bytes = metadata::identity_size(&identity, limits.manifest_bytes)?;
    for source in sources {
        harmos_runtime::bundle::validate_path(source.path)?;
        total = total.checked_add(source.bytes.len()).ok_or(Error::Limit("total file bytes"))?;
        if source.bytes.len() > limits.file_bytes || total > limits.total_file_bytes {
            return Err(Error::Limit("file bytes"));
        }
        if contents.insert(source.path.to_owned(), source.bytes).is_some() {
            return Err(invalid("duplicate file path"));
        }
        if source.executable.as_ref().is_some_and(|value| value.registrations.len() > limits.registrations) {
            return Err(Error::Limit("registration count"));
        }
        let sha256 = crate::digest(source.bytes);
        metadata::file_size(source, &sha256, !files.is_empty(), &mut metadata_bytes, limits.manifest_bytes)?;
        let mut executable = source.executable.clone();
        if let Some(executable) = &mut executable {
            executable.registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        }
        files.push(File {
            path: source.path.to_owned(),
            size: source.bytes.len() as u64,
            sha256,
            executable,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = Manifest { schema: 1, identity, files };
    let bytes = archive::encode(&manifest, &contents, limits)?;
    Bundle::inspect_with_settings(&bytes, limits, settings)
}

/* === Private Functions === */

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/* === Tests === */
