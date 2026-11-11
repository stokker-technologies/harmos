/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State File Ownership and Durable Replacement

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
// Local Imports
use super::format::{self, Read, corrupt, io};
use crate::state::{RecoverySnapshot, SnapshotPayload, SnapshotWriter};
use crate::{BackendFailure, Error, Result};

/* === Constants === */

const MANIFEST_BYTES: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

pub(super) type JournalOpener = Arc<dyn Fn(&Path) -> std::io::Result<File> + Send + Sync>;

/* === Structs === */

pub(super) struct Files {
    root: PathBuf,
    _lease: File,
    opener: JournalOpener,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Manifest {
    required_data: crate::StorageCursor,
    through: crate::RecoveryPosition,
    position: Option<harmos_domain::state::Position>,
    count: usize,
    total_bytes: usize,
    digest: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    digest: String,

    #[serde(with = "crate::state::payload")]
    payload: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    required_data: crate::StorageCursor,
    through: crate::RecoveryPosition,

    #[serde(with = "crate::state::payload")]
    payload: Vec<u8>,
}

/* === Enums === */

#[derive(Deserialize)]
#[serde(untagged)]
enum Opening {
    Manifest(Manifest),
    Legacy(Legacy),
}

/* === Traits === */

/* === Implementations === */

impl Files {
    #[tracing::instrument(name = "storage.state.file.open", level = "debug", skip_all)]
    pub(super) fn open(root: &Path, opener: JournalOpener) -> Result<Self> {
        fs::create_dir_all(root).map_err(io)?;
        let lease = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(root.join("state.lock")).map_err(io)?;
        lease.try_lock().map_err(|error| Error::backend("lock state storage", BackendFailure::Conflict, error))?;
        Ok(Self {
            root: root.to_owned(),
            _lease: lease,
            opener,
        })
    }

    #[tracing::instrument(name = "storage.state.file.journal", level = "debug", skip_all)]
    pub(super) fn journal(&self) -> Result<File> {
        (self.opener)(&self.root.join("state.jsonl")).map_err(io)
    }

    #[tracing::instrument(name = "storage.state.file.snapshot", level = "debug", skip_all)]
    pub(super) fn snapshot(&self, maximum: usize) -> Result<Option<RecoverySnapshot>> {
        let file = match File::open(self.root.join("state.json")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io(error)),
        };
        let mut reader = BufReader::new(file);
        let Read::Value(opening) = format::read::<Opening>(&mut reader, maximum.max(MANIFEST_BYTES))? else {
            return Err(corrupt("incomplete snapshot manifest"));
        };
        let manifest = match opening {
            Opening::Manifest(manifest) => manifest,
            Opening::Legacy(snapshot) => {
                format::encode(&snapshot, maximum)?;
                if !matches!(format::read::<serde_json::Value>(&mut reader, maximum)?, Read::End) {
                    return Err(corrupt("legacy snapshot has trailing records"));
                }
                let payload = SnapshotPayload::legacy(
                    &snapshot.payload,
                    std::num::NonZeroUsize::new(maximum).ok_or_else(|| corrupt("empty segment bound"))?,
                )
                .map_err(io)?;
                return Ok(Some(RecoverySnapshot {
                    required_data: snapshot.required_data,
                    through: snapshot.through,
                    payload,
                }));
            }
        };
        let mut writer =
            SnapshotWriter::new(std::num::NonZeroUsize::new(maximum).ok_or_else(|| corrupt("empty segment bound"))?).map_err(io)?;
        for _ in 0..manifest.count {
            let Read::Value(segment) = format::read::<Segment>(&mut reader, maximum)? else {
                return Err(corrupt("incomplete snapshot segment"));
            };
            if segment.payload.is_empty()
                || Sha256::digest(&segment.payload).iter().map(|byte| format!("{byte:02x}")).collect::<String>() != segment.digest
            {
                return Err(corrupt("snapshot segment digest differs"));
            }
            writer.write_all(&segment.payload).map_err(io)?;
            writer.finish_segment();
        }
        let Read::Value(committed) = format::read::<Manifest>(&mut reader, MANIFEST_BYTES)? else {
            return Err(corrupt("missing snapshot commit manifest"));
        };
        if committed != manifest || !matches!(format::read::<serde_json::Value>(&mut reader, MANIFEST_BYTES)?, Read::End) {
            return Err(corrupt("snapshot commit manifest differs or has trailing records"));
        }
        let payload = writer.finish(manifest.position.ok_or_else(|| corrupt("snapshot has no logical position"))?).map_err(io)?;
        if payload.len() != manifest.total_bytes
            || payload.manifest().count != manifest.count
            || payload.manifest_digest() != manifest.digest
        {
            return Err(corrupt("snapshot manifest size differs"));
        }
        payload.verify(maximum).map_err(io)?;
        Ok(Some(RecoverySnapshot {
            required_data: manifest.required_data,
            through: manifest.through,
            payload,
        }))
    }

    #[tracing::instrument(name = "storage.state.file.write_snapshot", level = "debug", skip_all)]
    pub(super) fn write_snapshot(&self, value: &RecoverySnapshot, maximum: usize) -> Result<()> {
        let payload = &value.payload;
        payload.verify(maximum).map_err(io)?;
        if payload.manifest().position.is_none() {
            return Err(super::format::invalid("snapshot publication requires a logical position"));
        }
        let manifest = Manifest {
            required_data: value.required_data.clone(),
            through: value.through.clone(),
            position: payload.manifest().position,
            count: payload.manifest().count,
            total_bytes: payload.len(),
            digest: payload.manifest_digest(),
        };
        self.replace("state.json", |writer| {
            format::write(writer, &manifest, MANIFEST_BYTES)?;
            let mut reader = payload.reader().map_err(io)?;
            for descriptor in &payload.manifest().segments {
                let mut bytes = vec![0; descriptor.bytes];
                reader.read_exact(&mut bytes).map_err(io)?;
                let segment = Segment {
                    digest: descriptor.digest.iter().map(|byte| format!("{byte:02x}")).collect(),
                    payload: bytes,
                };
                format::write(writer, &segment, maximum)?;
            }
            format::write(writer, &manifest, MANIFEST_BYTES)
        })
    }

    #[tracing::instrument(name = "storage.state.file.replace_journal", level = "debug", skip_all)]
    pub(super) fn replace_journal(&self, write: impl FnOnce(&mut BufWriter<File>) -> Result<()>) -> Result<()> {
        self.replace("state.jsonl", write)
    }

    // Directory Flush Is A POSIX Idiom: Windows Refuses A Directory Opened As A File
    // (`ERROR_ACCESS_DENIED`), And NTFS Makes Rename Metadata Durable Without One.
    #[tracing::instrument(name = "storage.state.file.sync_directory", level = "debug", skip_all)]
    pub(super) fn sync_directory(&self) -> Result<()> {
        if cfg!(unix) { File::open(&self.root).map_err(io)?.sync_all().map_err(io) } else { Ok(()) }
    }

    #[tracing::instrument(name = "storage.state.file.replace", level = "debug", skip_all)]
    fn replace(&self, name: &str, write: impl FnOnce(&mut BufWriter<File>) -> Result<()>) -> Result<()> {
        // The Exclusive Lease Owns These Temporary Names, Including Crash Remnants.
        let temporary = self.root.join(format!("{name}.pending"));
        let file = OpenOptions::new().create(true).truncate(true).write(true).open(&temporary).map_err(io)?;
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer.flush().map_err(io)?;
        writer.get_ref().sync_all().map_err(io)?;
        drop(writer);
        fs::rename(&temporary, self.root.join(name)).map_err(io)?;
        self.sync_directory()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
