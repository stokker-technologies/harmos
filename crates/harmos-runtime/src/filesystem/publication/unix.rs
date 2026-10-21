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
    fs::{self, File},
    io,
    path::Path,
};
// Third Party Imports

// Local Imports
use super::{ConfirmError, confirm_error, parent, sync_file};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn create_dir_all(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        sync_directory(path)?;
        return sync_directory(parent(path));
    }
    create_dir_all(parent(path))?;
    match fs::create_dir(path) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => (),
        Err(error) => return Err(error),
    }
    sync_directory(path)?;
    sync_directory(parent(path))
}

pub(super) fn move_directory(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "destination already exists"));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(source, destination)?;
    File::open(parent(destination))?.sync_all()?;
    if parent(source) != parent(destination) {
        File::open(parent(source))?.sync_all()?;
    }
    Ok(())
}

pub(super) fn install(source: &Path, destination: &Path) -> io::Result<()> {
    fs::hard_link(source, destination)?;
    sync_directory(parent(destination))
}

pub(super) fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)?;
    sync_directory(parent(destination))
}

pub(super) fn remove(path: &Path) -> io::Result<()> {
    // Refuse An Unavailable Durability Handle Before Unlinking Owned Bytes.
    let directory = match File::open(parent(path)) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    match fs::remove_file(path) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    // Retry Also Confirms An Unlink Whose Earlier Flush Failed.
    directory.sync_all()
}

pub(super) fn confirm_with_checkpoint<E>(path: &Path, checkpoint: &mut impl FnMut() -> Result<(), E>) -> Result<(), ConfirmError<E>> {
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    sync_file(path).map_err(|error| confirm_error(path, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)?;
    sync_directory(parent(path)).map_err(|error| confirm_error(path, error))?;
    checkpoint().map_err(ConfirmError::Checkpoint)
}

/* === Private Functions === */

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/* === Tests === */
