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
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::*;

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
pub(super) struct Journal {
    root: PathBuf,
    lock: Mutex<u64>,
}

#[derive(Serialize, Deserialize)]
struct Commit {
    #[serde(default)]
    pruned: Vec<u64>,
    #[serde(default)]
    release: Option<Release>,
    through: StorageCursor,
    entries: Vec<Entry>,
}

/* === Enums === */
#[derive(Serialize, Deserialize)]
enum Entry {
    Schema {
        position: StorageCursor,
        key: SchemaKey,
        root: usize,
        definitions: Vec<Vec<harmos_domain::schema::graph::Field>>,
    },
    Object(ObjectEntry),
    Record(StoredRecord),
    Row(StoredRow),
    File(FileReference),
}

/* === Traits === */
/* === Implementations === */
impl Journal {
    #[allow(
        dead_code,
        reason = "Shared fixture: only journal-backed story binaries restore persisted data"
    )]
    pub(super) fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root.join("staging")).map_err(io_error)?;
        fs::create_dir_all(root.join("files")).map_err(io_error)?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("journal.jsonl"))
            .map_err(io_error)?;
        #[cfg(unix)]
        File::open(root)
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
        Ok(Self {
            root: root.to_owned(),
            lock: Mutex::new(0),
        })
    }

    #[allow(
        dead_code,
        reason = "Shared fixture: only journal-backed story binaries restore persisted data"
    )]
    pub(super) fn restore<C: ObjectCatalog>(&self, data: &Data<C>) -> Result<()> {
        let mut frontier = self.lock.lock().unwrap();
        let path = self.root.join("journal.jsonl");
        let bytes = fs::read(&path).map_err(io_error)?;
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        // Only A Final Incomplete Record Is Uncommitted.
        if complete != bytes.len() {
            OpenOptions::new()
                .write(true)
                .open(&path)
                .map_err(io_error)?
                .set_len(complete as u64)
                .map_err(io_error)?;
        }
        for line in bytes[..complete]
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let commit: Commit =
                serde_json::from_slice(line).map_err(|_| failure(BackendFailure::Corrupt))?;
            data.history.check(&commit.through.history)?;
            if let Some(release) = commit.release {
                if release.through != commit.through
                    || release.through.sequence > data.saved.load(Ordering::SeqCst) as u64
                    || !commit.entries.is_empty()
                {
                    return Err(failure(BackendFailure::Corrupt));
                }
                upsert_release(&mut data.object_releases.lock().unwrap(), release);
                continue;
            }
            if !commit.pruned.is_empty() {
                if commit.through.sequence > data.saved.load(Ordering::SeqCst) as u64
                    || !commit.entries.is_empty()
                {
                    return Err(failure(BackendFailure::Corrupt));
                }
                if !commit.pruned.is_empty() {
                    data.control
                        .lost_objects_through
                        .fetch_max(commit.through.sequence as usize, Ordering::SeqCst);
                }
                for position in commit.pruned {
                    data.objects.lock().unwrap().remove(&position);
                }
                continue;
            }
            let mut expected = data.saved.load(Ordering::SeqCst) as u64 + 1;
            for entry in commit.entries {
                let position = match &entry {
                    Entry::Schema { position, .. } => position,
                    Entry::Object(value) => &value.position,
                    Entry::Record(value) => &value.position,
                    Entry::Row(value) => &value.record.position,
                    Entry::File(value) => &value.position,
                };
                if position.sequence != expected {
                    return Err(failure(BackendFailure::Corrupt));
                }
                data.history.check(&position.history)?;
                expected += 1;
                match entry {
                    Entry::Schema {
                        position,
                        key,
                        root,
                        definitions,
                    } => {
                        data.schemas.lock().unwrap().insert(
                            position.sequence,
                            StorageSchema::new(
                                position,
                                key,
                                harmos_domain::schema::graph::Schema { root, definitions },
                            ),
                        );
                    }
                    Entry::Object(value) => {
                        C::validate_value(&value.value, NonZeroUsize::MAX)?;
                        C::definitions().try_for_each(|definition| {
                            if definition.schema()? == value.value.schema {
                                data.object_schemas.lock().unwrap().insert(
                                    (
                                        value.value.schema.id.to_string(),
                                        value.value.schema.version.get(),
                                    ),
                                    (definition.declaration().fields)().to_vec(),
                                );
                            }
                            Ok::<(), harmos_storage::Error>(())
                        })?;
                        data.objects
                            .lock()
                            .unwrap()
                            .insert(value.position.sequence, value);
                    }
                    Entry::Record(value) => {
                        data.records
                            .lock()
                            .unwrap()
                            .insert(value.position.sequence, value);
                    }
                    Entry::Row(value) => {
                        data.rows
                            .lock()
                            .unwrap()
                            .insert(value.record.position.sequence, value);
                    }
                    Entry::File(value) => {
                        let bytes = fs::read(self.root.join("files").join(hex(&value.sha256)))
                            .map_err(io_error)?;
                        if bytes.len() as u64 != value.length
                            || <[u8; 32]>::from(Sha256::digest(&bytes)) != value.sha256
                        {
                            return Err(failure(BackendFailure::Corrupt));
                        }
                        data.blobs
                            .lock()
                            .unwrap()
                            .insert(value.metadata.id.to_string(), (value.clone(), bytes));
                        data.files
                            .lock()
                            .unwrap()
                            .insert(value.position.sequence, value);
                    }
                }
            }
            if expected != commit.through.sequence + 1 {
                return Err(failure(BackendFailure::Corrupt));
            }
            data.saved
                .store(commit.through.sequence as usize, Ordering::SeqCst);
        }
        *frontier = data.saved.load(Ordering::SeqCst) as u64;
        // These Stories Have No Resumable Producer; Unreferenced Staging Is Disposable.
        for entry in fs::read_dir(self.root.join("staging")).map_err(io_error)? {
            fs::remove_file(entry.map_err(io_error)?.path()).map_err(io_error)?;
        }
        Ok(())
    }

    pub(super) fn commit(&self, batch: StorageBatch<'_>) -> Result<()> {
        let mut frontier = self.lock.lock().unwrap();
        if batch.through().sequence <= *frontier {
            return Ok(());
        }
        if batch.first().sequence != *frontier + 1 {
            return Err(failure(BackendFailure::Corrupt));
        }
        let entries = batch
            .entries()
            .map(|entry| match entry {
                StorageEntry::Schema(value) => Entry::Schema {
                    position: value.position.clone(),
                    key: value.key.clone(),
                    root: value.schema.root,
                    definitions: value.schema.definitions.clone(),
                },
                StorageEntry::Object(value) => Entry::Object(value.clone()),
                StorageEntry::Record(value) => Entry::Record(value.clone()),
                StorageEntry::Row(value) => Entry::Row(value.clone()),
                StorageEntry::File(value) => Entry::File(value.clone()),
            })
            .collect();
        let commit = Commit {
            pruned: vec![],
            release: None,
            through: batch.through().clone(),
            entries,
        };
        let mut encoded =
            serde_json::to_vec(&commit).map_err(|_| failure(BackendFailure::Corrupt))?;
        encoded.push(b'\n');
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.root.join("journal.jsonl"))
            .map_err(io_error)?;
        file.write_all(&encoded).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        *frontier = batch.through().sequence;
        Ok(())
    }

    pub(super) fn prune(&self, through: &StorageCursor, positions: &[u64]) -> Result<()> {
        if positions.is_empty() {
            return Ok(());
        }
        let _frontier = self.lock.lock().unwrap();
        let record = Commit {
            pruned: positions.to_vec(),
            release: None,
            through: through.clone(),
            entries: vec![],
        };
        let mut bytes =
            serde_json::to_vec(&record).map_err(|_| failure(BackendFailure::Corrupt))?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.root.join("journal.jsonl"))
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)
    }

    pub(super) fn release(&self, release: &Release) -> Result<()> {
        let _frontier = self.lock.lock().unwrap();
        let record = Commit {
            pruned: vec![],
            release: Some(release.clone()),
            through: release.through.clone(),
            entries: vec![],
        };
        let mut bytes =
            serde_json::to_vec(&record).map_err(|_| failure(BackendFailure::Corrupt))?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.root.join("journal.jsonl"))
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)
    }

    pub(super) fn stage(&self, id: &Identifier, bytes: &[u8]) -> Result<()> {
        fs::write(self.staging(id), bytes).map_err(io_error)
    }

    pub(super) fn finish(&self, id: &Identifier, bytes: &[u8], digest: &[u8; 32]) -> Result<()> {
        let temporary = self.staging(id);
        let mut file = File::create(&temporary).map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        let destination = self.root.join("files").join(hex(digest));
        if destination.exists() {
            if fs::read(&destination).map_err(io_error)? != bytes {
                return Err(failure(BackendFailure::Corrupt));
            }
            fs::remove_file(temporary).map_err(io_error)?;
        } else {
            fs::rename(temporary, destination).map_err(io_error)?;
        }
        #[cfg(unix)]
        File::open(self.root.join("files"))
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
        Ok(())
    }

    fn staging(&self, id: &Identifier) -> PathBuf {
        self.root
            .join("staging")
            .join(hex(&Sha256::digest(id.as_str().as_bytes()).into()))
    }
}

/* === Public Functions === */
/* === Private Functions === */
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn io_error(_: std::io::Error) -> Error {
    failure(BackendFailure::Permanent)
}
/* === Tests === */
