/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Local Paths Are Accessible To The Host Without Traversing Symbolic Links.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::{self, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

// Third Party Imports

// Local Imports
use crate::arrow::RetainedMemory;
use crate::capacity::{Held, Pool, Refusal, Reservation};
use crate::manager::{ENTRY_OVERHEAD, Inner};
use crate::{BackendFailure, Error, Result};
use crate::{ByteRange, FileBatch, FileCompletion};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct LocalReader {
    context: crate::execution::Context,
    inner: Arc<Inner>,
    held: Held,
    file: Arc<Mutex<fs::File>>,
    path: Arc<PathBuf>,
    identity: Arc<same_file::Handle>,
    initial: Stamp,
    offset: u64,
    end: u64,
    bytes: u64,
    complete: bool,
    _metadata: Arc<Reservation>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl LocalReader {
    pub(crate) async fn open(inner: &Arc<Inner>, path: PathBuf, range: ByteRange, held: Held) -> Result<Self> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.local.open", outcome = "cancelled"));
        context
            .result(span, async {
                let absolute = std::path::absolute(path).map_err(io_error)?;
                let path_bytes = absolute.capacity().saturating_mul(3);
                let charge = path_bytes.saturating_add(std::mem::size_of::<Self>() + ENTRY_OVERHEAD);
                let metadata = Arc::new(Reservation::wait(inner, Pool::Read, &held, charge, None).await.map_err(Refusal::into_error)?);
                let memory = Arc::clone(&metadata);
                let (path, file, identity, initial, end) = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let _memory = memory;
                            let path = validate_local_path(&absolute)?;
                            if path.capacity().saturating_add(absolute.capacity()) > path_bytes {
                                return Err(Error::ResourceExhausted { resource: "local file path memory" });
                            }
                            if !fs::symlink_metadata(&path).map_err(io_error)?.is_file() {
                                return Err(Error::InvalidInput { field: "local file", reason: "source is not a regular file" });
                            }
                            let file = fs::File::open(&path).map_err(io_error)?;
                            let description = file.metadata().map_err(io_error)?;
                            if !description.is_file() {
                                return Err(Error::InvalidInput { field: "local file", reason: "source is not a regular file" });
                            }
                            let end = range.end(description.len())?;
                            let identity = same_file::Handle::from_file(file.try_clone().map_err(io_error)?).map_err(io_error)?;
                            let initial = Stamp::of(&description)?;
                            Ok::<_, Error>((path, file, identity, initial, end))
                        })
                    })
                }
                .await
                .map_err(task_error)??;
                Ok(Self {
                    context: crate::execution::Context::current(),
                    inner: Arc::clone(inner),
                    held,
                    file: Arc::new(Mutex::new(file)),
                    path: Arc::new(path),
                    identity: Arc::new(identity),
                    initial,
                    offset: range.offset,
                    end,
                    bytes: 0,
                    complete: false,
                    _metadata: metadata,
                })
            })
            .await
    }

    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn length(&self) -> u64 {
        self.end
    }

    pub(crate) fn start(&mut self, offset: u64) -> Result<()> {
        if offset > self.end {
            return Err(Error::InvalidInput { field: "file offset", reason: "offset exceeds file length" });
        }
        self.offset = offset;
        Ok(())
    }

    pub(crate) fn retarget(&mut self, end: u64) {
        self.end = end;
        self.complete = false;
    }

    pub(crate) async fn inspect(&self) -> Result<Option<(bool, u64)>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::debug_span!("storage.local.inspect", outcome = "cancelled"));
        context
            .result(span, async {
                let path = Arc::clone(&self.path);
                let identity = Arc::clone(&self.identity);
                let memory = Arc::clone(&self._metadata);
                {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let _memory = memory;
                            match fs::symlink_metadata(path.as_ref()) {
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                                Err(error) => return Err(io_error(error)),
                                Ok(metadata) if !metadata.is_file() => {
                                    return Err(Error::InvalidInput { field: "local file", reason: "source is not a regular file" });
                                }
                                Ok(_) => {}
                            }
                            let path = validate_local_path(&path)?;
                            let file = fs::File::open(path).map_err(io_error)?;
                            let length = file.metadata().map_err(io_error)?.len();
                            let current = same_file::Handle::from_file(file).map_err(io_error)?;
                            Ok::<_, Error>(Some((*identity == current, length)))
                        })
                    })
                }
                .await
                .map_err(task_error)?
            })
            .await
    }

    pub(crate) async fn check_unchanged(&self) -> Result<()> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::debug_span!("storage.local.check_unchanged", outcome = "cancelled"));
        context
            .result(span, async {
                let path = Arc::clone(&self.path);
                let file = Arc::clone(&self.file);
                let identity = Arc::clone(&self.identity);
                let initial = self.initial;
                let memory = Arc::clone(&self._metadata);
                {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let _memory = memory;
                            let opened = file.lock().unwrap().metadata().map_err(io_error)?;
                            let current_file = fs::File::open(validate_local_path(&path)?).map_err(io_error)?;
                            let current = current_file.metadata().map_err(io_error)?;
                            let current_identity = same_file::Handle::from_file(current_file).map_err(io_error)?;
                            if *identity != current_identity || Stamp::of(&opened)? != initial || Stamp::of(&current)? != initial {
                                return Err(Error::InvalidInput {
                                    field: "capture source",
                                    reason: "source changed during stable capture",
                                });
                            }
                            Ok(())
                        })
                    })
                }
                .await
                .map_err(task_error)?
            })
            .await
    }

    pub(crate) async fn next(&mut self, event: bool) -> Result<Option<(FileBatch, RetainedMemory)>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::debug_span!("storage.local.next", outcome = "cancelled"));
        context
            .result(span, async {
                if self.offset == self.end {
                    self.complete = true;
                    return Ok(None);
                }
                let ceiling = self.inner.settings.response.batch_bytes.get();
                let size = crate::file_batch::batch_limit(ceiling, self.offset, self.end - self.offset, event)?.get();
                let charge = size.saturating_add(std::mem::size_of::<FileBatch>() + ENTRY_OVERHEAD);
                let transfer =
                    Reservation::wait(&self.inner, Pool::Read, &self.held, charge, None).await.map_err(Refusal::into_error)?.retain();
                let file = Arc::clone(&self.file);
                let offset = self.offset;
                // The Blocking Job Owns Its Charge Even If The Awaiting Reader Is Cancelled.
                let (batch, transfer) = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let mut file = file.lock().unwrap();
                            file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
                            let mut bytes = vec![0; size];
                            file.read_exact(&mut bytes).map_err(io_error)?;
                            Ok::<_, Error>((FileBatch { offset, bytes }, transfer))
                        })
                    })
                }
                .await
                .map_err(task_error)??;
                self.offset += batch.bytes.len() as u64;
                self.bytes += batch.bytes.len() as u64;
                Ok(Some((batch, transfer)))
            })
            .await
    }

    pub(crate) fn completion(&self) -> Result<FileCompletion> {
        if !self.complete {
            return Err(Error::NotReady { domain: "file read completion" });
        }
        Ok(FileCompletion { bytes: self.bytes })
    }
}

impl Stamp {
    fn of(metadata: &Metadata) -> Result<Self> {
        Ok(Self { length: metadata.len(), modified: metadata.modified().map_err(io_error)?, created: metadata.created().ok() })
    }
}

/* === Public Functions === */

pub(crate) fn validate_local_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).map_err(io_error)?;
    let mut checked = PathBuf::new();
    for component in absolute.components() {
        if component == Component::ParentDir {
            checked.pop();
            continue;
        }
        if component == Component::CurDir {
            continue;
        }
        checked.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&checked).map_err(io_error)?;
        if linked(&metadata) {
            return Err(Error::InvalidInput { field: "local path", reason: "symbolic link traversal is forbidden" });
        }
    }
    Ok(checked)
}

/* === Private Functions === */

pub(crate) fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(crate) fn io_error(error: std::io::Error) -> Error {
    Error::backend("local filesystem", BackendFailure::Permanent, error)
}

fn task_error(error: tokio::task::JoinError) -> Error {
    Error::backend("local filesystem task", BackendFailure::Permanent, error)
}

/* === Tests === */
