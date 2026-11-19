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
use std::fs::{self, File};
use std::io::Read;
// Third Party Imports
use sha2::{Digest, Sha256};
use tempfile::TempDir;
// Local Imports
use super::{
    Error, PluginManagerSettings,
    persistence::{SavedEntry, problem},
};
use crate::{
    bundle::{self, Bundle, Facility, Manifest},
    plugin::grpc::Managed,
};
use harmos_protocol::host;
use harmos_storage::{FileReference, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Installation {
    directory: TempDir,
    executable: String,
    digest: [u8; 32],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Installation {
    pub async fn prepare(storage: &StorageManager, managed: &host::PluginManaged, settings: &PluginManagerSettings) -> Result<Self, Error> {
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
        let (executable, digest) = if bundle.manifest().launch.is_some() {
            // The Reserved Manifest Is the Verified Entry Point for Command Bundles.
            let manifest = serde_json::to_vec(bundle.manifest()).map_err(|error| problem("Encode Installed Manifest", error))?;
            ("manifest.json".to_owned(), Sha256::digest(manifest).into())
        } else {
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
            let digest = Sha256::digest(bundle.file(&executable).expect("verified executable exists")).into();
            (executable, digest)
        };
        fs::create_dir_all(&settings.directory).map_err(|error| problem("Create Plugin Directory", error))?;
        let directory = bundle.extract(&settings.directory).map_err(|error| problem("Install Verified Plugin Bundle", error))?;
        if let Some(launch) = &bundle.manifest().launch {
            Managed::launch(launch, directory.path()).map_err(|error| {
                Error::new("launch_program", error.to_string(), &format!("install {} at its declared location and retry", launch[0]))
            })?;
        }
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

pub(super) fn version(saved: &SavedEntry, settings: &PluginManagerSettings) -> Result<String, Error> {
    let executable = saved
        .executable
        .as_ref()
        .ok_or_else(|| Error::new("executable_missing", "Read Managed Plugin Version", "recreate the entry from a verified bundle"))?;
    bundle::validate_path(executable).map_err(|error| problem("Verify Installed Executable Path", error))?;
    let parent = executable.split('/').next().expect("installation path has a parent");
    let path = settings.directory.join(parent).join("manifest.json");
    let file = File::open(path).map_err(|error| problem("Read Installed Manifest", error))?;
    let mut bytes = Vec::new();
    file.take((settings.bundle.manifest_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| problem("Read Installed Manifest", error))?;
    if bytes.len() > settings.bundle.manifest_bytes {
        return Err(Error::new(
            "manifest_capacity",
            "Read Installed Manifest",
            "reinstall a bundle within the configured manifest capacity",
        ));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| problem("Read Installed Manifest", error))?;
    Ok(manifest.identity.version.to_string())
}

pub(super) fn remove(executable: &str, settings: &PluginManagerSettings) -> Result<(), Error> {
    bundle::validate_path(executable).map_err(|error| problem("Delete Plugin Installation", error))?;
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
    fs::remove_dir_all(directory).map_err(|error| problem("Delete Plugin Installation", error))
}

/* === Private Functions === */

/* === Tests === */
