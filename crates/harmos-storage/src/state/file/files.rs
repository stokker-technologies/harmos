/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State File Ownership And Durable Replacement

use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};

use super::format::{self, Read, corrupt, io};
use crate::{BackendFailure, Error, Result};

pub(super) type JournalOpener = Arc<dyn Fn(&Path) -> std::io::Result<File> + Send + Sync>;

pub(super) struct Files {
    root: PathBuf,
    _lease: File,
    opener: JournalOpener,
}

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
        if !matches!(format::read::<serde_json::Value>(&mut reader, maximum)?, Read::End) {
            return Err(corrupt("state checkpoint contains trailing records"));
        }
        Ok(Some(value))
    }

    #[tracing::instrument(name = "storage.state.file.write_snapshot", level = "debug", skip_all)]
    pub(super) fn write_snapshot<T: Serialize>(&self, value: &T, maximum: usize) -> Result<()> {
        self.replace("state.json", |writer| format::write(writer, value, maximum))
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
