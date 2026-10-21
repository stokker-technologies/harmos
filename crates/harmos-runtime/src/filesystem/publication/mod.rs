/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(unix)]
mod unix;

// The Windows Publication Primitives Are A Win32 FFI Boundary; Each Call Carries Its SAFETY Argument.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
// Third Party Imports
use tempfile::NamedTempFile;
// Local Imports
#[cfg(unix)]
use unix as platform;

#[cfg(windows)]
use windows as platform;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Staging(PathBuf);

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum ConfirmError<E> {
    #[error(transparent)]
    Filesystem(crate::filesystem::Error),

    #[error(transparent)]
    Checkpoint(E),
}

/* === Traits === */

/* === Implementations === */

impl Staging {
    fn file(directory: &Path) -> io::Result<(Self, File)> {
        let temporary = NamedTempFile::new_in(directory)?;
        let (file, path) = temporary.keep().map_err(|error| error.error)?;
        Ok((Self(path), file))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/* === Public Functions === */

pub fn create_dir_all(path: &Path) -> crate::filesystem::Result<()> {
    platform::create_dir_all(path).map_err(|error| crate::filesystem::Error::publication("create directory", path, error))
}

pub fn move_directory(source: &Path, destination: &Path) -> crate::filesystem::Result<()> {
    let metadata =
        fs::symlink_metadata(source).map_err(|error| crate::filesystem::Error::publication("inspect directory", source, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(crate::filesystem::Error::publication(
            "move directory",
            source,
            io::Error::other("source must be a regular directory"),
        ));
    }
    platform::move_directory(source, destination)
        .map_err(|error| crate::filesystem::Error::publication("move directory", destination, error))
}

pub fn install(source: &Path, destination: &Path) -> crate::filesystem::Result<()> {
    sync_file(source).map_err(|error| crate::filesystem::Error::publication("flush staged file", source, error))?;
    platform::install(source, destination).map_err(|error| crate::filesystem::Error::publication("install file", destination, error))
}

pub fn replace(source: &Path, destination: &Path) -> crate::filesystem::Result<()> {
    sync_file(source).map_err(|error| crate::filesystem::Error::publication("flush staged file", source, error))?;
    platform::replace(source, destination).map_err(|error| crate::filesystem::Error::publication("replace file", destination, error))
}

pub fn remove(path: &Path) -> crate::filesystem::Result<()> {
    match fs::symlink_metadata(parent(path)) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(crate::filesystem::Error::publication(
                "remove file",
                path,
                io::Error::new(io::ErrorKind::InvalidInput, "parent must be a regular directory"),
            ));
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(crate::filesystem::Error::publication("inspect parent", path, error));
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(crate::filesystem::Error::publication(
                "remove file",
                path,
                io::Error::new(io::ErrorKind::InvalidInput, "source must be a regular file"),
            ));
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => {
            return Err(crate::filesystem::Error::publication("inspect file", path, error));
        }
    }
    platform::remove(path).map_err(|error| crate::filesystem::Error::publication("remove file", path, error))
}

pub fn confirm(path: &Path) -> crate::filesystem::Result<()> {
    match confirm_with_checkpoint(path, || Ok::<(), std::convert::Infallible>(())) {
        Ok(()) => Ok(()),
        Err(ConfirmError::Filesystem(error)) => Err(error),
        Err(ConfirmError::Checkpoint(error)) => match error {},
    }
}

pub fn confirm_with_checkpoint<E>(path: &Path, mut checkpoint: impl FnMut() -> Result<(), E>) -> Result<(), ConfirmError<E>> {
    platform::confirm_with_checkpoint(path, &mut checkpoint)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn write(path: &Path, bytes: &[u8]) -> crate::filesystem::Result<()> {
    write_document(path, bytes).map_err(|error| crate::filesystem::Error::publication("write document", path, error))
}

/* === Private Functions === */

fn confirm_error<E>(path: &Path, error: io::Error) -> ConfirmError<E> {
    let error = crate::filesystem::Error::publication("confirm file durability", path, error);
    ConfirmError::Filesystem(error)
}

fn write_document(path: &Path, bytes: &[u8]) -> io::Result<()> {
    create_dir_all(parent(path))?;
    let (staging, mut file) = Staging::file(parent(path))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    replace(&staging.0, path).map_err(Into::into)
}

fn sync_file(path: &Path) -> io::Result<()> {
    // FlushFileBuffers requires GENERIC_WRITE. Read-only File::open is not a
    // portable way to confirm file durability, even if no bytes are changed.
    OpenOptions::new().read(true).write(true).open(path)?.sync_all()
}

fn parent(path: &Path) -> &Path {
    path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

/* === Tests === */
