/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Local Paths Are Accessible To The Host Without Traversing Symbolic Links.

use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::manager::Inner;
use crate::{BackendFailure, Error, Result};
use crate::{ByteRange, FileBatch, FileCompletion};

pub(crate) struct LocalReader {
    context: crate::execution::Context,
    inner: Arc<Inner>,

    file: Arc<fs::File>,
    path: Option<Arc<PathBuf>>,
    identity: Arc<same_file::Handle>,
    initial: Stamp,
    offset: u64,
    end: u64,
    bytes: u64,
    complete: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

impl LocalReader {
    pub(crate) async fn open(inner: &Arc<Inner>, path: PathBuf, range: ByteRange) -> Result<Self> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.local.open", outcome = "cancelled"));
        context
            .result(span, async {
                let absolute = std::path::absolute(path).map_err(io_error)?;

                let (path, file, identity, initial, end) = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let path = validate_local_path(&absolute)?;
                            if !fs::symlink_metadata(&path).map_err(io_error)?.is_file() {
                                return Err(Error::InvalidInput {
                                    field: "local file",
                                    reason: "source is not a regular file",
                                });
                            }
                            let file = fs::File::open(&path).map_err(io_error)?;
                            let description = file.metadata().map_err(io_error)?;
                            if !description.is_file() {
                                return Err(Error::InvalidInput {
                                    field: "local file",
                                    reason: "source is not a regular file",
                                });
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

                    file: Arc::new(file),
                    path: Some(Arc::new(path)),
                    identity: Arc::new(identity),
                    initial,
                    offset: range.offset,
                    end,
                    bytes: 0,
                    complete: false,
                })
            })
            .await
    }

    pub(crate) async fn opened(inner: &Arc<Inner>, file: fs::File, range: ByteRange) -> Result<Self> {
        let (file, identity, initial, end) = tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            let file = independent_file(file)?;
            let description = file.metadata().map_err(io_error)?;
            if !description.is_file() {
                return Err(Error::InvalidInput {
                    field: "opened file",
                    reason: "source is not a regular file",
                });
            }
            let end = range.end(description.len())?;
            let identity = same_file::Handle::from_file(file.try_clone().map_err(io_error)?).map_err(io_error)?;
            let initial = Stamp::of(&description)?;
            Ok((file, identity, initial, end))
        })
        .await
        .map_err(task_error)??;
        Ok(Self {
            context: crate::execution::Context::current(),
            inner: Arc::clone(inner),

            file: Arc::new(file),
            path: None,
            identity: Arc::new(identity),
            initial,
            offset: range.offset,
            end,
            bytes: 0,
            complete: false,
        })
    }

    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn length(&self) -> u64 {
        self.end
    }

    pub(crate) fn observation(&self) -> (u64, SystemTime) {
        (self.initial.length, self.initial.modified)
    }

    pub(crate) async fn observation_unchanged(&self) -> Result<bool> {
        let file = Arc::clone(&self.file);
        let initial = self.observation();
        tokio::task::spawn_blocking(move || {
            let current = file.metadata().map_err(io_error)?;
            Ok::<_, Error>((current.len(), current.modified().map_err(io_error)?) == initial)
        })
        .await
        .map_err(task_error)?
    }

    pub(crate) fn start(&mut self, offset: u64) -> Result<()> {
        if offset > self.end {
            return Err(Error::InvalidInput {
                field: "file offset",
                reason: "offset exceeds file length",
            });
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
                let path = self.path.clone();
                let identity = Arc::clone(&self.identity);
                {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let path = path.as_ref().ok_or(Error::InvalidInput {
                                field: "file inspection",
                                reason: "opened source has no pathname",
                            })?;
                            match fs::symlink_metadata(path.as_ref()) {
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                                Err(error) => return Err(io_error(error)),
                                Ok(metadata) if !metadata.is_file() => {
                                    return Err(Error::InvalidInput {
                                        field: "local file",
                                        reason: "source is not a regular file",
                                    });
                                }
                                Ok(_) => {}
                            }
                            let path = validate_local_path(path)?;
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
                let path = self.path.clone();
                let file = Arc::clone(&self.file);
                let identity = Arc::clone(&self.identity);
                let initial = self.initial;
                {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let opened = file.metadata().map_err(io_error)?;
                            if path.is_none() {
                                if Stamp::of(&opened)? != initial {
                                    return Err(Error::InvalidInput {
                                        field: "capture source",
                                        reason: "source changed during stable capture",
                                    });
                                }
                                return Ok(());
                            }
                            let path = path.as_ref().expect("path checked");
                            let current_file = fs::File::open(validate_local_path(path)?).map_err(io_error)?;
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

    pub(crate) async fn next(&mut self, event: bool) -> Result<Option<FileBatch>> {
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

                let file = Arc::clone(&self.file);
                let offset = self.offset;
                // The Blocking Job Owns Its Charge Even If The Awaiting Reader Is Cancelled.
                let batch = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let mut bytes = vec![0; size];
                            read_exact_at(&file, &mut bytes, offset).map_err(io_error)?;
                            Ok::<_, Error>(FileBatch { offset, bytes })
                        })
                    })
                }
                .await
                .map_err(task_error)??;
                self.offset += batch.bytes.len() as u64;
                self.bytes += batch.bytes.len() as u64;
                Ok(Some(batch))
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
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().map_err(io_error)?,
            created: metadata.created().ok(),
        })
    }
}

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
            return Err(Error::InvalidInput {
                field: "local path",
                reason: "symbolic link traversal is forbidden",
            });
        }
    }
    Ok(checked)
}

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

fn read_exact_at(file: &fs::File, mut bytes: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    while !bytes.is_empty() {
        #[cfg(unix)]
        let result = {
            use std::os::unix::fs::FileExt;
            file.read_at(bytes, offset)
        };
        #[cfg(windows)]
        let result = {
            use std::os::windows::fs::FileExt;
            file.seek_read(bytes, offset)
        };
        #[cfg(not(any(unix, windows)))]
        let result: std::io::Result<usize> = {
            let _ = (file, offset);
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "positional file reads are unavailable"))
        };
        match result {
            Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "opened source ended before selected range")),
            Ok(count) => {
                offset = offset
                    .checked_add(count as u64)
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "file offset overflow"))?;
                bytes = &mut bytes[count..];
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "ReOpenFile preserves the supplied file object while creating a separately owned cursor")]
fn independent_file(file: fs::File) -> Result<fs::File> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_SHARING_VIOLATION, GENERIC_READ, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, ReOpenFile},
    };

    // Reopen The Supplied Object For An Independent Cursor. Read-Only Access
    // Meets Reader Needs; Sharing Delete Keeps Delete-On-Close Sources Usable.
    // SAFETY: The Original Owned Handle Stays Live During This Synchronous Call.
    let original = file.as_raw_handle();
    let share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    let reopened = unsafe { ReOpenFile(original, GENERIC_READ, share_mode, 0) };
    if reopened == INVALID_HANDLE_VALUE {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) {
            return Err(Error::InvalidInput {
                field: "opened file",
                reason: "source handle does not permit an independent read handle",
            });
        }
        return Err(io_error(error));
    }
    // SAFETY: ReOpenFile Returned A Fresh Owned Handle; File Closes It Once.
    let reopened = unsafe { fs::File::from_raw_handle(reopened) };
    drop(file);
    Ok(reopened)
}

fn task_error(error: tokio::task::JoinError) -> Error {
    Error::backend("local filesystem task", BackendFailure::Permanent, error)
}
