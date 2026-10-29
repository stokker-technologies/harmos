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
    diagnostic: Arc<PathBuf>,
    frozen: Option<Arc<crate::frozen_file::Backing>>,
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

#[derive(Debug, thiserror::Error)]
#[error("{step} at `{path}`: {source}")]
struct LocalIoError {
    step: &'static str,
    path: PathBuf,

    #[source]
    source: std::io::Error,
}

impl LocalReader {
    pub(crate) async fn open(inner: &Arc<Inner>, path: PathBuf, range: ByteRange) -> Result<Self> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.local.open", outcome = "cancelled"));
        context
            .result(span, async {
                let absolute = std::path::absolute(&path).map_err(|error| io_error("resolve path", &path, error))?;

                let (path, file, identity, initial, end) = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let path = validate_local_path(&absolute)?;
                            if !fs::symlink_metadata(&path).map_err(|error| io_error("inspect path", &path, error))?.is_file() {
                                return Err(Error::InvalidInput {
                                    field: "local file",
                                    reason: "source is not a regular file",
                                });
                            }
                            let file = fs::File::open(&path).map_err(|error| io_error("open source", &path, error))?;
                            let description = file.metadata().map_err(|error| io_error("inspect source", &path, error))?;
                            if !description.is_file() {
                                return Err(Error::InvalidInput {
                                    field: "local file",
                                    reason: "source is not a regular file",
                                });
                            }
                            let end = range.end(description.len())?;
                            let identity =
                                same_file::Handle::from_file(file.try_clone().map_err(|error| io_error("clone source", &path, error))?)
                                    .map_err(|error| io_error("identify source", &path, error))?;
                            let initial = Stamp::of(&description, &path)?;
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
                    diagnostic: Arc::new(path.clone()),
                    path: Some(Arc::new(path)),
                    frozen: None,
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

    pub(crate) async fn opened(
        inner: &Arc<Inner>, file: fs::File, range: ByteRange, frozen: Option<Arc<crate::frozen_file::Backing>>,
    ) -> Result<Self> {
        let (file, path, identity, initial, end, frozen) = tokio::task::spawn_blocking(move || {
            let path = opened_path(&file);
            #[cfg(windows)]
            let file = independent_file(file, &path)?;
            let description = file.metadata().map_err(|error| io_error("inspect opened source", &path, error))?;
            if !description.is_file() {
                return Err(Error::InvalidInput {
                    field: "opened file",
                    reason: "source is not a regular file",
                });
            }
            let end = range.end(description.len())?;
            let identity = same_file::Handle::from_file(file.try_clone().map_err(|error| io_error("clone opened source", &path, error))?)
                .map_err(|error| io_error("identify opened source", &path, error))?;
            let initial = Stamp::of(&description, &path)?;
            Ok((file, path, identity, initial, end, frozen))
        })
        .await
        .map_err(task_error)??;
        Ok(Self {
            context: crate::execution::Context::current(),
            inner: Arc::clone(inner),

            file: Arc::new(file),
            path: None,
            diagnostic: Arc::new(path),
            frozen,
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
        let path = self.diagnostic.clone();
        let frozen = self.frozen.clone();
        tokio::task::spawn_blocking(move || {
            let _frozen = frozen;
            let current = file.metadata().map_err(|error| io_error("inspect opened source", path.as_ref(), error))?;
            Ok::<_, Error>(
                (current.len(), current.modified().map_err(|error| io_error("read modification time", path.as_ref(), error))?) == initial,
            )
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
                                Err(error) => return Err(io_error("inspect path", path.as_ref(), error)),
                                Ok(metadata) if !metadata.is_file() => {
                                    return Err(Error::InvalidInput {
                                        field: "local file",
                                        reason: "source is not a regular file",
                                    });
                                }
                                Ok(_) => {}
                            }
                            let path = validate_local_path(path)?;
                            let file = fs::File::open(&path).map_err(|error| io_error("open source", &path, error))?;
                            let length = file.metadata().map_err(|error| io_error("inspect source", &path, error))?.len();
                            let current = same_file::Handle::from_file(file).map_err(|error| io_error("identify source", &path, error))?;
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
                let diagnostic = self.diagnostic.clone();
                let frozen = self.frozen.clone();
                {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let _frozen = frozen;
                            let opened = file.metadata().map_err(|error| io_error("inspect source", diagnostic.as_ref(), error))?;
                            if path.is_none() {
                                if Stamp::of(&opened, &diagnostic)? != initial {
                                    return Err(Error::InvalidInput {
                                        field: "capture source",
                                        reason: "source changed during stable capture",
                                    });
                                }
                                return Ok(());
                            }
                            let path = path.as_ref().expect("path checked");
                            let current_file = fs::File::open(validate_local_path(path)?)
                                .map_err(|error| io_error("open current source", path.as_ref(), error))?;
                            let current =
                                current_file.metadata().map_err(|error| io_error("inspect current source", path.as_ref(), error))?;
                            let current_identity = same_file::Handle::from_file(current_file)
                                .map_err(|error| io_error("identify current source", path.as_ref(), error))?;
                            if *identity != current_identity
                                || Stamp::of(&opened, &diagnostic)? != initial
                                || Stamp::of(&current, path)? != initial
                            {
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
                let path = self.diagnostic.clone();
                let frozen = self.frozen.clone();
                // The Blocking Job Retains Its File And Buffer Through Caller Cancellation.
                let batch = {
                    let context = crate::execution::Context::current();
                    tokio::task::spawn_blocking(move || {
                        context.blocking(|| {
                            let _frozen = frozen;
                            let mut bytes = vec![0; size];
                            read_exact_at(&file, &mut bytes, offset).map_err(|error| io_error("read source", path.as_ref(), error))?;
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
    fn of(metadata: &Metadata, path: &Path) -> Result<Self> {
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().map_err(|error| io_error("read modification time", path, error))?,
            created: metadata.created().ok(),
        })
    }
}

pub(crate) fn validate_local_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).map_err(|error| io_error("resolve path", path, error))?;
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
        let metadata = fs::symlink_metadata(&checked).map_err(|error| io_error("inspect path component", &checked, error))?;
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

pub(crate) fn io_error(step: &'static str, path: &Path, source: std::io::Error) -> Error {
    let context = LocalIoError { step, path: path.to_owned(), source };
    Error::backend("local filesystem", BackendFailure::Permanent, context)
}

// Diagnostic Only: Never Use A Resolved Handle Path To Reopen The Source.
#[allow(unsafe_code, reason = "Windows queries the live owned handle into a separately allocated UTF-16 buffer")]
pub(crate) fn opened_path(file: &fs::File) -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{FILE_NAME_OPENED, GetFinalPathNameByHandleW};

        // SAFETY: The Handle Stays Live; A Zero-Length Query Writes No Bytes.
        let length = unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), std::ptr::null_mut(), 0, FILE_NAME_OPENED) };
        if length != 0 {
            let mut buffer = vec![0u16; length as usize];
            // SAFETY: The Buffer Owns The Advertised Number Of UTF-16 Elements.
            let written = unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), buffer.as_mut_ptr(), length, FILE_NAME_OPENED) };
            if written != 0 && written < length {
                return std::ffi::OsString::from_wide(&buffer[..written as usize]).into();
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;

        let descriptor = file.as_raw_fd();
        for directory in ["/proc/self/fd", "/dev/fd"] {
            let link = Path::new(directory).join(descriptor.to_string());
            if let Ok(path) = fs::read_link(link) {
                return path;
            }
        }
    }
    PathBuf::from("<opened file; path unavailable>")
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
fn independent_file(file: fs::File, path: &Path) -> Result<fs::File> {
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
        return Err(io_error("reopen independent source", path, error));
    }
    // SAFETY: ReOpenFile Returned A Fresh Owned Handle; File Closes It Once.
    let reopened = unsafe { fs::File::from_raw_handle(reopened) };
    drop(file);
    Ok(reopened)
}

fn task_error(error: tokio::task::JoinError) -> Error {
    Error::backend("local filesystem task", BackendFailure::Permanent, error)
}
