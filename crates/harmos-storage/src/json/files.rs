/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Independent Native File Staging, Publication And Bounded Reads.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex as AsyncMutex, RwLock};

use super::format;
use crate::{FileCheckpoint, FileMetadata, FilePhase, FileReference, FileSession, Result, StagedFile, StorageCursor, StoreHistory};

pub(super) struct Files {
    pub root: PathBuf,
    pub history: StoreHistory,
    pub maximum: usize,
    gates: Mutex<BTreeMap<String, Weak<AsyncMutex<()>>>>,
    pub activity: Arc<RwLock<()>>,
    pub closing: AtomicBool,
    pub lease: Mutex<Weak<File>>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Production<C = FileCheckpoint> {
    pub metadata: FileMetadata,
    pub generation: u64,
    pub checkpoint: Option<C>,
    pub finish: Option<FileReference>,
    pub phase: FilePhase,
    pub deleted: bool,
}

pub(super) struct Reader {
    pub reference: FileReference,
    file: File,
}

impl Files {
    pub fn new(root: PathBuf, history: StoreHistory, maximum: usize) -> Self {
        Self {
            root,
            history,
            maximum,
            gates: Mutex::new(BTreeMap::new()),
            activity: Arc::new(RwLock::new(())),
            closing: AtomicBool::new(false),
            lease: Mutex::new(Weak::new()),
        }
    }

    pub async fn call<T: Send + 'static>(
        self: &Arc<Self>, id: &crate::Identifier, action: impl FnOnce(&Self, &str) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        if self.closing.load(Ordering::Acquire) {
            return Err(crate::Error::Closing { domain: "JSON storage" });
        }
        let activity = tracing::Instrument::instrument(self.activity.clone().read_owned(), tracing::trace_span!("storage.lock.wait")).await;
        if self.closing.load(Ordering::Acquire) {
            return Err(crate::Error::Closing { domain: "JSON storage" });
        }
        let lease = self
            .lease
            .lock()
            .map_err(|_| format::corrupt("file lease is poisoned"))?
            .upgrade()
            .ok_or(crate::Error::NotReady { domain: "JSON storage" })?;
        let key = key(id);
        let gate = {
            let mut gates = self.gates.lock().map_err(|_| format::corrupt("file gate is poisoned"))?;
            gates.retain(|_, gate| gate.strong_count() > 0);
            match gates.get(&key).and_then(Weak::upgrade) {
                Some(gate) => gate,
                None => {
                    let gate = Arc::new(AsyncMutex::new(()));
                    gates.insert(key.clone(), Arc::downgrade(&gate));
                    gate
                }
            }
        };
        let guard = tracing::Instrument::instrument(gate.lock_owned(), tracing::trace_span!("storage.lock.wait")).await;
        let files = self.clone();
        {
            let context = crate::execution::Context::current();
            tokio::task::spawn_blocking(move || {
                context.blocking(|| {
                    let _activity = activity;
                    let _lease = lease;
                    let _guard = guard;
                    action(&files, &key)
                })
            })
        }
        .await
        .map_err(super::task)?
    }

    pub fn metadata_path(&self, key: &str) -> PathBuf {
        self.root.join("productions").join(format!("{key}.json"))
    }
    pub fn stage_path(&self, key: &str) -> PathBuf {
        self.root.join("staging").join(key)
    }
    pub fn final_path(&self, key: &str) -> PathBuf {
        self.root.join("files").join(key)
    }

    pub fn load(&self, key: &str) -> Result<Production> {
        let value =
            format::load::<Production>(&self.metadata_path(key), self.maximum)?.ok_or_else(|| format::invalid("unknown staged file"))?;
        if value.deleted {
            return Err(format::conflict("file has been deleted"));
        }
        Ok(value)
    }

    pub fn save<C: Serialize>(&self, key: &str, production: &Production<C>) -> Result<()> {
        format::store(&self.metadata_path(key), production, self.maximum)
    }

    pub fn create(&self, key: &str, metadata: FileMetadata, public: bool) -> Result<StagedFile> {
        if let Some(stored) = format::load::<Production>(&self.metadata_path(key), self.maximum)? {
            if stored.deleted || stored.metadata != metadata || (stored.generation != 0) != public || stored.phase != FilePhase::Producing {
                return Err(format::conflict("file identity has different metadata or ownership"));
            }
        } else {
            let generation = u64::from(public);
            let checkpoint = public.then(|| {
                FileCheckpoint::new(
                    metadata.clone(),
                    FileSession {
                        history: self.history.clone(),
                        id: metadata.id.clone(),
                        generation,
                    },
                    0,
                    &Sha256::new(),
                )
            });
            let production = Production {
                metadata: metadata.clone(),
                generation,
                checkpoint,
                finish: None,
                phase: FilePhase::Producing,
                deleted: false,
            };
            let path = self.stage_path(key);
            format::regular(&path)?;
            let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path).map_err(format::io)?;
            if file.metadata().map_err(format::io)?.len() != 0 {
                return Err(format::conflict("unowned staging contains bytes"));
            }
            file.sync_all().map_err(format::io)?;
            format::sync_directory(&self.root.join("staging"))?;
            self.save(key, &production)?;
        }
        Ok(StagedFile {
            history: self.history.clone(),
            id: metadata.id,
        })
    }

    pub fn write(&self, key: &str, offset: u64, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Err(format::invalid("file chunk is empty"));
        }

        let production = self.load(key)?;
        if production.phase == FilePhase::Abandoned {
            return Err(format::conflict("file is abandoned"));
        }
        let sealed = production.finish.is_some();
        let path = if sealed && self.final_path(key).exists() { self.final_path(key) } else { self.stage_path(key) };
        format::regular(&path)?;
        let mut file = OpenOptions::new().read(true).write(!sealed).open(path).map_err(format::io)?;
        let length = file.metadata().map_err(format::io)?.len();
        let end = offset.checked_add(bytes.len() as u64).ok_or_else(|| format::invalid("file offset overflow"))?;
        if offset > length || sealed && end > length || production.metadata.expected_length.is_some_and(|expected| end > expected) {
            return Err(format::conflict("file write has a gap or exceeds declared length"));
        }
        let overlap = (length - offset).min(bytes.len() as u64) as usize;
        file.seek(SeekFrom::Start(offset)).map_err(format::io)?;
        let mut existing = vec![0_u8; 1024 * 1024];
        let mut checked = 0;
        while checked < overlap {
            let count = (overlap - checked).min(existing.len());
            file.read_exact(&mut existing[..count]).map_err(format::io)?;
            if existing[..count] != bytes[checked..checked + count] {
                return Err(format::conflict("file retry bytes differ"));
            }
            checked += count;
        }
        if !sealed {
            file.write_all(&bytes[overlap..]).map_err(format::io)?;
        }
        Ok(())
    }

    pub fn finish(&self, key: &str, position: StorageCursor, length: u64, sha256: [u8; 32]) -> Result<FileReference> {
        self.history.check(&position.history)?;
        if position.sequence == 0 {
            return Err(format::invalid("file position is zero"));
        }

        let mut production = self.load(key)?;
        let reference = FileReference {
            position,
            metadata: production.metadata.clone(),
            length,
            sha256,
        };
        if production.phase == FilePhase::Abandoned
            || production.finish.as_ref().is_some_and(|prior| prior != &reference)
            || reference.metadata.expected_length.is_some_and(|expected| expected != length)
            || reference.metadata.expected_sha256.is_some_and(|expected| expected != sha256)
        {
            return Err(format::conflict("file finish differs from metadata or prior finish"));
        }
        let final_path = self.final_path(key);
        let staged_path = self.stage_path(key);
        let present = format::regular(&final_path)?;
        let path = if present { &final_path } else { &staged_path };
        verify(path, length, sha256)?;
        let durable = OpenOptions::new().read(true).write(true).open(path).map_err(format::io)?;
        durable.sync_all().map_err(format::io)?;
        if production.finish.is_none() {
            production.finish = Some(reference.clone());
            self.save(key, &production)?;
        }
        if !present {
            fs::rename(&staged_path, &final_path).map_err(format::io)?;
        }
        drop(durable);
        format::sync_directory(&self.root.join("files"))?;
        format::sync_directory(&self.root.join("staging"))?;
        production.phase = FilePhase::Finished;
        self.save(key, &production)?;
        Ok(reference)
    }

    // Validate Once Before The Snapshot Owns The Immutable Native Handle.
    pub fn open_reader(&self, key: &str, reference: FileReference) -> Result<Reader> {
        let production = self.load(key)?;
        if production.deleted || production.phase != FilePhase::Finished || production.finish.as_ref() != Some(&reference) {
            return Err(format::conflict("immutable file reference differs"));
        }
        let path = self.final_path(key);
        format::regular(&path)?;
        let file = File::open(path).map_err(format::io)?;
        if file.metadata().map_err(format::io)?.len() != reference.length {
            return Err(format::corrupt("immutable file length differs"));
        }
        Ok(Reader { reference, file })
    }

    pub fn read(&self, key: &str, offset: u64, maximum: NonZeroUsize, reference: Option<&FileReference>) -> Result<Vec<u8>> {
        let length = if let Some(reference) = reference {
            let production = format::load::<Production>(&self.metadata_path(key), self.maximum)?
                .ok_or_else(|| format::invalid("unknown staged file"))?;
            if production.deleted {
                return Err(format::conflict("file has been deleted"));
            }
            if production.phase != FilePhase::Finished || production.finish.as_ref() != Some(reference) {
                return Err(format::conflict("immutable file reference differs"));
            }
            reference.length
        } else {
            self.progress(key)?.length
        };
        if offset > length {
            return Err(format::invalid("read exceeds file length"));
        }
        let path = if self.final_path(key).exists() { self.final_path(key) } else { self.stage_path(key) };
        format::regular(&path)?;
        let mut file = File::open(path).map_err(format::io)?;
        let physical = file.metadata().map_err(format::io)?.len();
        if physical < length || reference.is_some() && physical != length {
            return Err(format::corrupt("file is shorter than its saved prefix"));
        }
        file.seek(SeekFrom::Start(offset)).map_err(format::io)?;
        let count = (length - offset).min(maximum.get() as u64) as usize;
        let mut bytes = vec![0; count];
        file.read_exact(&mut bytes).map_err(format::io)?;
        Ok(bytes)
    }

    pub fn abort(&self, key: &str, abandon: bool, delete: bool) -> Result<()> {
        let mut production =
            format::load::<Production>(&self.metadata_path(key), self.maximum)?.ok_or_else(|| format::invalid("unknown staged file"))?;
        if production.deleted && !delete {
            return Err(format::conflict("file has been deleted"));
        }
        if production.phase == FilePhase::Finished && !delete {
            return Ok(());
        }
        if production.generation != 0 && !abandon && !delete {
            return Err(format::conflict("public production must be abandoned before deletion"));
        }
        if abandon || production.generation == 0 {
            production.phase = FilePhase::Abandoned;
        }
        if delete {
            production.deleted = true;
        }
        self.save(key, &production)?;
        if production.generation == 0 || delete {
            for path in [self.stage_path(key), self.final_path(key)] {
                if format::regular(&path)? {
                    fs::remove_file(path).map_err(format::io)?;
                }
            }
        }
        format::sync_directory(&self.root.join("staging"))?;
        format::sync_directory(&self.root.join("files"))
    }
}

impl Reader {
    pub fn read(&mut self, offset: u64, maximum: NonZeroUsize) -> Result<Vec<u8>> {
        if offset > self.reference.length {
            return Err(format::invalid("read exceeds file length"));
        }
        let count = (self.reference.length - offset).min(maximum.get() as u64) as usize;
        self.file.seek(SeekFrom::Start(offset)).map_err(format::io)?;
        let mut bytes = vec![0; count];
        self.file.read_exact(&mut bytes).map_err(format::io)?;
        Ok(bytes)
    }
}

pub(super) fn key(id: &crate::Identifier) -> String {
    format::hex(&Sha256::digest(id.as_str().as_bytes()))
}

pub(super) fn verify(path: &Path, length: u64, expected: [u8; 32]) -> Result<()> {
    format::regular(path)?;
    let mut file = File::open(path).map_err(format::io)?;
    if file.metadata().map_err(format::io)?.len() != length {
        return Err(format::corrupt("file length differs"));
    }
    let mut buffer = vec![0; 1024 * 1024];
    let mut digest = Sha256::new();
    loop {
        let count = file.read(&mut buffer).map_err(format::io)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if <[u8; 32]>::from(digest.finalize()) != expected {
        return Err(format::corrupt("file checksum differs"));
    }
    Ok(())
}
