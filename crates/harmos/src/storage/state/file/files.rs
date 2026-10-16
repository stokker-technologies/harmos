/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State File Ownership And Durable Replacement

/* === Modules === */
/* === Re-Exports === */
/* === Imports === */
// Standard Library Imports
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};
// Local Imports
use super::format::{self, Read, corrupt, io};
use crate::{BackendFailure, Error, Result};

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
pub(super) struct Files {
    root: PathBuf,
    _lease: File,
}

/* === Enums === */
/* === Traits === */
/* === Implementations === */
impl Files {
    pub(super) fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(io)?;
        let lease = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("state.lock"))
            .map_err(io)?;
        lease.try_lock().map_err(|error| {
            Error::backend("lock state storage", BackendFailure::Conflict, error)
        })?;
        Ok(Self {
            root: root.to_owned(),
            _lease: lease,
        })
    }

    pub(super) fn journal(&self) -> Result<File> {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("state.jsonl"))
            .map_err(io)
    }

    pub(super) fn snapshot<T: DeserializeOwned>(&self, maximum: usize) -> Result<Option<T>> {
        let file = match File::open(self.root.join("state.json")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io(error)),
        };
        let mut reader = BufReader::new(file);
        let value = match format::read(&mut reader, maximum)? {
            Read::Value(value) => value,
            Read::End | Read::Incomplete => return Err(corrupt("incomplete state checkpoint")),
        };
        if !matches!(
            format::read::<serde_json::Value>(&mut reader, maximum)?,
            Read::End
        ) {
            return Err(corrupt("state checkpoint contains trailing records"));
        }
        Ok(Some(value))
    }

    pub(super) fn write_snapshot<T: Serialize>(&self, value: &T, maximum: usize) -> Result<()> {
        self.replace("state.json", |writer| format::write(writer, value, maximum))
    }

    /// The caller has already durably published a checkpoint covering discarded entries.
    pub(super) fn replace_journal(
        &self,
        write: impl FnOnce(&mut BufWriter<File>) -> Result<()>,
    ) -> Result<()> {
        self.replace("state.jsonl", write)
    }

    pub(super) fn sync_directory(&self) -> Result<()> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_FLAG_BACKUP_SEMANTICS permits opening a directory handle.
            options.custom_flags(0x02000000);
        }
        options.open(&self.root).map_err(io)?.sync_all().map_err(io)
    }

    fn replace(
        &self,
        name: &str,
        write: impl FnOnce(&mut BufWriter<File>) -> Result<()>,
    ) -> Result<()> {
        // The Exclusive Lease Owns These Temporary Names, Including Crash Remnants.
        let temporary = self.root.join(format!("{name}.pending"));
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .map_err(io)?;
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
