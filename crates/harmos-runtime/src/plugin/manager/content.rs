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
use std::{future::Future, pin::Pin};
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::{Error, PluginManagerSettings, persistence::problem};
use crate::bundle::{Bundle, Facility};
use crate::storage::{ByteRange, FileRead, FileReference, FileSource, StorageManager};
use harmos_protocol::v1 as wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type ContentRead<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

/* === Structs === */

pub struct PublishedContent(pub StorageManager);

pub(super) struct Installation {
    directory: tempfile::TempDir,
    executable: String,
    digest: [u8; 32],
}

/* === Enums === */

/* === Traits === */

pub trait ContentSource: Send + Sync + 'static {
    fn read<'a>(&'a self, content: &'a wire::PluginManaged, max_bytes: usize) -> ContentRead<'a>;
}

/* === Implementations === */

impl ContentSource for PublishedContent {
    fn read<'a>(&'a self, content: &'a wire::PluginManaged, max_bytes: usize) -> ContentRead<'a> {
        Box::pin(async move {
            let reference: FileReference = content
                .content
                .clone()
                .ok_or_else(|| {
                    Error::new("content_required", "Create Managed Plugin", "publish the bundle through an upload session first")
                })?
                .try_into()
                .map_err(|error| problem("Read Published Bundle", error))?;
            if reference.length > max_bytes as u64 {
                return Err(Error::new(
                    "bundle_capacity",
                    "Read Published Bundle",
                    "upload a bundle within the configured archive capacity",
                ));
            }
            let mut reader = self
                .0
                .read_file(FileRead {
                    source: FileSource::Stored(reference),
                    range: ByteRange { offset: 0, limit: None },
                })
                .await
                .map_err(|error| problem("Read Published Bundle", error))?;
            let mut bytes = Vec::new();
            while let Some(batch) = reader.next().await.map_err(|error| problem("Read Published Bundle", error))? {
                if batch.bytes().len() > max_bytes.saturating_sub(bytes.len()) {
                    return Err(Error::new(
                        "bundle_capacity",
                        "Read Published Bundle",
                        "upload a bundle within the configured archive capacity",
                    ));
                }
                bytes.extend_from_slice(batch.bytes());
            }
            reader.completion().map_err(|error| problem("Verify Published Bundle", error))?;
            Ok(bytes)
        })
    }
}

impl Installation {
    pub async fn prepare(
        source: &dyn ContentSource, managed: &wire::PluginManaged, settings: &PluginManagerSettings,
    ) -> Result<Self, Error> {
        let bytes = source.read(managed, settings.bundle.archive_bytes).await?;
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

/* === Private Functions === */

/* === Tests === */
