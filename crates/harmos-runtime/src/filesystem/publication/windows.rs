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
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    mem,
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
};
// Third Party Imports
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, GENERIC_WRITE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO_EX, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfoEx,
        FileRenameInfoEx, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, ReOpenFile, SetFileInformationByHandle,
    },
    System::WindowsProgramming::{FILE_RENAME_FLAG_POSIX_SEMANTICS, FILE_RENAME_FLAG_REPLACE_IF_EXISTS},
};
// Local Imports
use super::{ConfirmError, Staging, confirm_error, parent};

/* === Constants === */
const COPY_BYTES: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn create_dir_all(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    let parent = parent(path);
    if parent == path {
        return Err(io::Error::new(io::ErrorKind::NotFound, "storage volume does not exist"));
    }
    create_dir_all(parent)?;
    let temporary = tempfile::Builder::new().prefix(".").tempdir_in(parent)?.keep();
    let result = move_file(&temporary, path, false);
    // Only our empty directory can be removed. Never recursively remove an
    // ancestor or another writer's directory, including after a failed move.
    let _ = fs::remove_dir(&temporary);
    match result {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        other => other,
    }
}

pub(super) fn move_directory(source: &Path, destination: &Path) -> io::Result<()> {
    move_file(source, destination, false)
}

pub(super) fn install(source: &Path, destination: &Path) -> io::Result<()> {
    move_file(source, destination, false)
}

pub(super) fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    move_file(source, destination, true)
}

pub(super) fn remove(path: &Path) -> io::Result<()> {
    // Keep A Writable File Handle Across The Namespace Removal.
    // A Missing File Is Recreated Without Truncating Existing Bytes So A Retry
    // Can Confirm A Prior Unlink Whose Flush Failed, Without Opening A Directory.
    let pinned = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let metadata = pinned.metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "source must be a regular file"));
    }
    // Reopen The Verified Object, Not Its Potentially Replaced Path.
    // SAFETY: The Original Owned Handle Stays Live During This Synchronous Call.
    let deletion = unsafe {
        ReOpenFile(pinned.as_raw_handle(), DELETE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, FILE_FLAG_OPEN_REPARSE_POINT)
    };
    if deletion == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ReOpenFile Returned A Fresh Owned Handle; File Closes It Once.
    let deletion = unsafe { File::from_raw_handle(deletion) };
    let disposition = FILE_DISPOSITION_INFO_EX { Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS };
    // SAFETY: The Owned Handle And Initialized Fixed-Size Disposition Remain
    // Valid For This Synchronous Call. Unsupported POSIX Removal Is An Error.
    if unsafe {
        SetFileInformationByHandle(
            deletion.as_raw_handle(),
            FileDispositionInfoEx,
            (&disposition as *const FILE_DISPOSITION_INFO_EX).cast(),
            mem::size_of_val(&disposition) as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // Closing The POSIX Delete Handle Removes The Name While Other Handles
    // Retain The Bytes. Flush The Remaining Writable Handle After That Removal.
    drop(deletion);
    pinned.sync_all()
}

pub(super) fn confirm_with_checkpoint<E>(path: &Path, checkpoint: &mut impl FnMut() -> Result<(), E>) -> Result<(), ConfirmError<E>> {
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    let mut existing = File::open(path).map_err(|error| confirm_error(path, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    let (staging, mut copy) = Staging::file(parent(path)).map_err(|error| confirm_error(path, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    let mut buffer = [0; COPY_BYTES];
    loop {
        checkpoint().map_err(ConfirmError::Checkpoint)?;
        let count = existing.read(&mut buffer).map_err(|error| confirm_error(path, error))?;
        checkpoint().map_err(ConfirmError::Checkpoint)?;
        if count == 0 {
            break;
        }
        copy.write_all(&buffer[..count]).map_err(|error| confirm_error(path, error))?;
        checkpoint().map_err(ConfirmError::Checkpoint)?;
    }
    copy.sync_all().map_err(|error| confirm_error(path, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    drop(copy);
    drop(existing);
    move_file_with_checkpoint(&staging.0, path, true, checkpoint)
}

/* === Private Functions === */
fn move_file(source: &Path, destination: &Path, replace: bool) -> io::Result<()> {
    match move_file_with_checkpoint(source, destination, replace, &mut || Ok::<(), std::convert::Infallible>(())) {
        Ok(()) => Ok(()),
        Err(ConfirmError::Filesystem(error)) => Err(error.into()),
        Err(ConfirmError::Checkpoint(error)) => match error {},
    }
}

fn move_file_with_checkpoint<E>(
    source: &Path, destination: &Path, replace: bool, checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), ConfirmError<E>> {
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    let destination_wide = wide(destination).map_err(|error| confirm_error(destination, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    // DELETE is the access right a rename needs and GENERIC_WRITE the one
    // FlushFileBuffers demands afterwards; the share mode leaves every other
    // reader and writer of the source undisturbed. BACKUP_SEMANTICS lets the
    // same handle open a directory, OPEN_REPARSE_POINT keeps a link as the
    // thing being renamed rather than its target.
    let handle = OpenOptions::new()
        .access_mode(DELETE | GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(source)
        .map_err(|error| confirm_error(destination, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    match rename_by_handle(&handle, &destination_wide, replace) {
        Ok(()) => {
            // The handle now names the destination; flushing it commits the
            // NTFS log records of the rename, which is what makes the new
            // name survive a power loss.
            checkpoint().map_err(ConfirmError::Checkpoint)?;
            handle.sync_all().map_err(|error| confirm_error(destination, error))?;
            checkpoint().map_err(ConfirmError::Checkpoint)
        }
        Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => {
            checkpoint().map_err(ConfirmError::Checkpoint)?;
            drop(handle);
            move_file_write_through(source, &destination_wide, replace).map_err(|error| confirm_error(destination, error))?;
            checkpoint().map_err(ConfirmError::Checkpoint)
        }
        Err(error) => Err(confirm_error(destination, error)),
    }
}

fn rename_by_handle(handle: &File, destination: &[u16], replace: bool) -> io::Result<()> {
    // FILE_RENAME_INFO ends in a flexible `FileName` array: allocate the fixed
    // header plus the name (the struct already reserves one u16 of it) in a
    // buffer aligned for the struct, the way the standard library lays it out.
    let name_bytes = mem::size_of_val(destination);
    let total = mem::size_of::<FILE_RENAME_INFO>() + name_bytes - mem::size_of::<u16>();
    let mut buffer = vec![0u64; total.div_ceil(mem::size_of::<u64>())];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    let mut flags = FILE_RENAME_FLAG_POSIX_SEMANTICS;
    if replace {
        flags |= FILE_RENAME_FLAG_REPLACE_IF_EXISTS;
    }
    // SAFETY: `buffer` holds at least `total` bytes at u64 alignment, which
    // satisfies FILE_RENAME_INFO; the header fields are written in place and
    // the name, NUL included, is copied into the flexible array that follows.
    unsafe {
        (*info).Anonymous.Flags = flags;
        (*info).RootDirectory = std::ptr::null_mut();
        // The length excludes the terminating NUL that `wide` appended.
        (*info).FileNameLength = (name_bytes - mem::size_of::<u16>()) as u32;
        std::ptr::copy_nonoverlapping(destination.as_ptr(), (*info).FileName.as_mut_ptr(), destination.len());
        if SetFileInformationByHandle(handle.as_raw_handle(), FileRenameInfoEx, info.cast(), total as u32) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn move_file_write_through(source: &Path, destination: &[u16], replace: bool) -> io::Result<()> {
    let source = wide(source)?;
    let flags = MOVEFILE_WRITE_THROUGH | if replace { MOVEFILE_REPLACE_EXISTING } else { 0 };
    // SAFETY: both paths are owned, NUL-terminated UTF-16 buffers valid for the
    // synchronous call; their interior NULs were rejected by wide().
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    // absolute() preserves verbatim/UNC paths, supports non-ASCII names, and
    // removes dependence on changes to the process current working directory.
    let path = std::path::absolute(path)?;
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "publication path must have a filename"))?;
    // canonicalize produces a verbatim Windows prefix, also for UNC shares.
    // Appending the possibly not-yet-existing name preserves long-path support.
    let path = fs::canonicalize(parent(&path))?.join(name);
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL"));
    }
    value.push(0);
    Ok(value)
}

/* === Tests === */
