/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verified Published Plugin Content

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::{Error, PluginManagerSettings, persistence::problem};
use crate::bundle::{Bundle, Facility};
use harmos_protocol::host as wire;
use harmos_storage::{FileReference, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Installation {
    directory: tempfile::TempDir,
    executable: String,
    digest: [u8; 32],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Installation {
    pub async fn prepare(storage: &StorageManager, managed: &wire::PluginManaged, settings: &PluginManagerSettings) -> Result<Self, Error> {
        let reference: FileReference = managed
            .content
            .clone()
            .ok_or_else(|| Error::new("content_required", "Create Managed Plugin", "supply an immutable content reference"))?
            .try_into()
            .map_err(|error| problem("Read Plugin Bundle", error))?;
        if reference.length > settings.bundle.archive_bytes as u64 {
            return Err(Error::new("bundle_capacity", "Read Plugin Bundle", "supply a bundle within the configured archive capacity"));
        }
        let read = storage.open_content(&reference, None, None).await.map_err(|error| problem("Read Plugin Bundle", error))?;
        let mut bytes = Vec::new();
        while (bytes.len() as u64) < reference.length {
            let range = read
                .get_bytes(bytes.len() as u64, storage.settings().response.batch_bytes)
                .await
                .map_err(|error| problem("Read Plugin Bundle", error))?;
            if range.data.is_empty() || range.data.len() > settings.bundle.archive_bytes.saturating_sub(bytes.len()) {
                return Err(Error::new("bundle_capacity", "Read Plugin Bundle", "read bounded nonempty content ranges"));
            }
            bytes.extend_from_slice(&range.data);
        }
        if bytes.len() as u64 != reference.length || Sha256::digest(&bytes).as_slice() != reference.sha256 {
            return Err(Error::new("content_digest", "Read Plugin Bundle", "restore the immutable content and its digest"));
        }
        if bytes.len() > settings.bundle.archive_bytes {
            return Err(Error::new("bundle_capacity", "Create Managed Plugin", "supply content within the configured archive capacity"));
        }
        if managed.sha256.len() != 32 || Sha256::digest(&bytes).as_slice() != managed.sha256 {
            return Err(Error::new(
                "bundle_digest",
                "Create Managed Plugin: Bundle Digest Differs",
                "upload the original bundle and supply its SHA-256 digest",
            ));
        }
        let bundle = Bundle::inspect(&bytes, settings.bundle).map_err(|error| problem("Verify Plugin Bundle", error))?;
        let mut executables = bundle.manifest().files.iter().filter(|file| {
            file.executable
                .as_ref()
                .is_some_and(|executable| executable.facility == Facility::Sidecar && executable.target == settings.target)
        });
        let executable = executables
            .next()
            .ok_or_else(|| {
                Error::new(
                    "executable_missing",
                    "Create Managed Plugin: Native Executable Is Missing",
                    "include one executable for the configured native target",
                )
            })?
            .path
            .clone();
        if executables.next().is_some() || bundle.file(&executable).is_none_or(|bytes| bytes.is_empty()) {
            return Err(Error::new(
                "executable_invalid",
                "Create Managed Plugin: Native Executable Is Invalid",
                "include exactly one nonempty executable for the native target",
            ));
        }
        std::fs::create_dir_all(&settings.directory).map_err(|error| problem("Create Plugin Directory", error))?;
        let digest = Sha256::digest(bundle.file(&executable).expect("verified executable exists")).into();
        let directory = bundle.extract(&settings.directory).map_err(|error| problem("Install Verified Plugin Bundle", error))?;
        let parent = directory.path().file_name().and_then(|name| name.to_str()).expect("installation directory has a UTF-8 name");
        let executable = format!("{parent}/{executable}");
        Ok(Self { directory, executable, digest })
    }
    pub fn executable(&self) -> &str {
        &self.executable
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn commit(self) {
        let _ = self.directory.keep();
    }
}

/* === Public Functions === */

pub(super) fn remove(executable: &str, settings: &PluginManagerSettings) -> Result<(), Error> {
    crate::bundle::validate_path(executable).map_err(|error| problem("Delete Plugin Installation", error))?;
    let parent = executable.split('/').next().expect("installation path has a parent");
    let root = settings.directory.canonicalize().map_err(|error| problem("Resolve Plugin Directory", error))?;
    let directory = root.join(parent).canonicalize().map_err(|error| problem("Resolve Plugin Installation", error))?;
    if directory.parent() != Some(root.as_path()) {
        return Err(Error::new(
            "installation_path",
            "Delete Plugin Installation",
            "restore an installation directly beneath the manager directory",
        ));
    }
    std::fs::remove_dir_all(directory).map_err(|error| problem("Delete Plugin Installation", error))
}

/* === Private Functions === */

/* === Tests === */
