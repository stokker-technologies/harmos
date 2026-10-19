/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Replay And Checkpoint Compaction

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom, Write};
use std::path::Path;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::files::Files;
use super::format::{self, Read, corrupt, invalid, io};
use crate::storage::state::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StorageStateSettings};
use crate::storage::{ReadLimits, RecoveryPosition, StoreHistory};
use crate::{BackendFailure, Error, Result};

/* === Constants === */

// Format 2 Encodes Payloads As Base64; Earlier Roots Are Refused, Never Migrated.
const VERSION: u32 = 2;
// Headers Hold Only A Position, So Their Bound Is Harmos-Owned.
const HEADER_BYTES: usize = 4096;
/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct StateLog {
    files: Files,
    file: File,
    poisoned: bool,
    history: StoreHistory,
    saved: u64,
    base: u64,
    entry_bytes: usize,
    checkpoint_bytes: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    version: u32,
    after: RecoveryPosition,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StateLog {
    pub(super) fn open(root: &Path, history: StoreHistory, settings: StorageStateSettings) -> Result<Self> {
        let entry_bytes = settings.max_entry_bytes.get();
        let checkpoint_bytes = settings.max_checkpoint_bytes.get();
        let files = Files::open(root)?;

        // The Journal Header Identifies The Format Before Any Payload Is Decoded.
        let mut file = files.journal()?;
        let created = file.metadata().map_err(io)?.len() == 0;
        if created {
            let header = Header { version: VERSION, after: RecoveryPosition { history: history.clone(), sequence: 0 } };
            format::write(&mut file, &header, HEADER_BYTES)?;
            file.sync_all().map_err(io)?;
            files.sync_directory()?;
        }
        let mut reader = BufReader::new(file.try_clone().map_err(io)?);
        reader.seek(SeekFrom::Start(0)).map_err(io)?;
        let header = header(&mut reader, &history)?;

        let snapshot: Option<RecoverySnapshot> = files.snapshot(checkpoint_bytes)?;
        let base = if let Some(snapshot) = snapshot {
            history.check(&snapshot.through.history)?;
            snapshot.through.sequence
        } else {
            0
        };
        if created && base != 0 {
            return Err(corrupt("state checkpoint has no journal"));
        }
        if header.after.sequence > base {
            return Err(corrupt("state journal begins beyond checkpoint"));
        }
        let mut saved = header.after.sequence;
        loop {
            let start = reader.stream_position().map_err(io)?;
            match format::read::<RecoveryEntry>(&mut reader, entry_bytes)? {
                Read::End => break,
                Read::Incomplete => {
                    file.set_len(start).map_err(io)?;
                    file.sync_all().map_err(io)?;
                    break;
                }
                Read::Value(entry) => {
                    history.check(&entry.position.history)?;
                    if saved.checked_add(1) != Some(entry.position.sequence) {
                        return Err(corrupt("state journal positions are not consecutive"));
                    }
                    saved = entry.position.sequence;
                }
            }
        }
        if saved < base {
            return Err(corrupt("state journal does not reach checkpoint"));
        }
        Ok(Self { files, file, poisoned: false, history, saved, base, entry_bytes, checkpoint_bytes })
    }

    pub(super) fn check(&self) -> Result<()> {
        if self.poisoned {
            return Err(io(std::io::Error::other("state journal requires close and reopen after an unresolved I/O failure")));
        }
        Ok(())
    }

    pub(super) fn position(&self) -> RecoveryPosition {
        RecoveryPosition { history: self.history.clone(), sequence: self.saved }
    }

    /// The caller bounds the snapshot value; the configured limit bounds the written record.
    pub(super) fn snapshot(&self, maximum: usize) -> Result<Option<RecoverySnapshot>> {
        let bound = maximum.saturating_add(format::OVERHEAD);
        self.files.snapshot(bound.min(self.checkpoint_bytes))
    }

    pub(super) fn page(&mut self, after: &RecoveryPosition, through: &RecoveryPosition, limits: ReadLimits) -> Result<RecoveryPage> {
        self.history.check(&after.history)?;
        self.history.check(&through.history)?;
        if after.sequence < self.base {
            return Err(Error::Gap { requested: after.sequence, available: self.base });
        }
        if after.sequence > through.sequence || through.sequence > self.saved {
            return Err(invalid("state read range is outside retained history"));
        }
        let mut entries = Vec::new();
        let mut bytes = 0usize;
        let mut last = after.sequence;
        self.scan(|entry| {
            if entry.position.sequence <= after.sequence {
                return Ok(true);
            }
            if entry.position.sequence > through.sequence {
                return Ok(false);
            }
            let size = crate::storage::manager::encoded_size(&entry)?;
            if entries.len() == limits.rows.get() || size > limits.bytes.get().saturating_sub(bytes) {
                if entries.is_empty() {
                    return Err(invalid("state entry exceeds requested page size"));
                }
                return Ok(false);
            }
            bytes += size;
            last = entry.position.sequence;
            entries.push(entry);
            Ok(true)
        })?;
        let next = (last < through.sequence).then(|| RecoveryPosition { history: self.history.clone(), sequence: last });
        let page = RecoveryPage { entries, next };
        page.check(after, through, limits, |entry| crate::storage::manager::encoded_size(entry).unwrap_or(usize::MAX))?;
        Ok(page)
    }

    pub(super) fn append(&mut self, entries: &[RecoveryEntry]) -> Result<()> {
        let mut next = self.saved;
        let mut lines = Vec::new();
        for entry in entries {
            self.history.check(&entry.position.history)?;
            // Encode Every Record Before Writing So An Oversized Entry Appends Nothing.
            let line = format::encode(entry, self.entry_bytes)?;
            let sequence = entry.position.sequence;
            if sequence <= self.saved {
                if sequence <= self.base {
                    return Err(invalid("state retry precedes retained journal"));
                }
                let mut same = false;
                self.scan(|stored| {
                    if stored.position.sequence == sequence {
                        same = stored == *entry;
                        return Ok(false);
                    }
                    Ok(stored.position.sequence < sequence)
                })?;
                if !same {
                    return Err(conflict("state retry has different content"));
                }
            } else {
                if next.checked_add(1) != Some(sequence) {
                    return Err(invalid("state append is not consecutive"));
                }
                next = sequence;
                lines.push(line);
            }
        }
        let start = self.file.seek(SeekFrom::End(0)).map_err(io)?;
        let result = (|| {
            for line in &lines {
                self.file.write_all(line).map_err(io)?;
            }
            self.file.sync_all().map_err(io)
        })();
        if let Err(error) = result {
            // Preserve The Acknowledged Prefix Before Permitting Another Append.
            self.poisoned = true;
            self.file.set_len(start).map_err(io)?;
            self.file.sync_all().map_err(io)?;
            self.poisoned = false;
            return Err(error);
        }
        self.saved = next;
        Ok(())
    }

    pub(super) fn flush(&mut self, through: &RecoveryPosition) -> Result<RecoveryPosition> {
        self.history.check(&through.history)?;
        if through.sequence > self.saved {
            return Err(invalid("state flush exceeds stored position"));
        }
        self.file.sync_all().map_err(io)?;
        Ok(self.position())
    }

    pub(super) fn checkpoint(&mut self, snapshot: &RecoverySnapshot) -> Result<()> {
        self.history.check(&snapshot.through.history)?;
        let base = snapshot.through.sequence;
        if base < self.base || base > self.saved {
            return Err(invalid("state checkpoint is outside stored history"));
        }
        if base == self.base
            && let Some(existing) = self.snapshot(self.checkpoint_bytes)?
            && existing != *snapshot
        {
            return Err(conflict("state checkpoint retry has different content"));
        }
        self.files.write_snapshot(snapshot, self.checkpoint_bytes)?;
        self.base = base;
        // A Durable Checkpoint Always Precedes Removal Of Its Covered Journal.
        let mut reader = self.reader()?;
        let history = self.history.clone();
        let maximum = self.entry_bytes;
        let replaced = self.files.replace_journal(|writer| {
            format::write(writer, &Header { version: VERSION, after: snapshot.through.clone() }, HEADER_BYTES)?;
            loop {
                match format::read::<RecoveryEntry>(&mut reader, maximum)? {
                    Read::End => break,
                    Read::Incomplete => {
                        return Err(corrupt("live state journal has incomplete tail"));
                    }
                    Read::Value(entry) => {
                        history.check(&entry.position.history)?;
                        if entry.position.sequence > base {
                            format::write(writer, &entry, maximum)?;
                        }
                    }
                }
            }
            Ok(())
        });
        // Rename May Succeed Before Directory Sync Fails; Always Reopen The Named File.
        self.poisoned = true;
        self.file = self.files.journal()?;
        self.poisoned = false;
        replaced
    }

    fn reader(&self) -> Result<BufReader<File>> {
        let mut reader = BufReader::new(self.file.try_clone().map_err(io)?);
        reader.seek(SeekFrom::Start(0)).map_err(io)?;
        header(&mut reader, &self.history)?;
        Ok(reader)
    }

    fn scan(&self, mut visit: impl FnMut(RecoveryEntry) -> Result<bool>) -> Result<()> {
        let mut reader = self.reader()?;
        loop {
            match format::read::<RecoveryEntry>(&mut reader, self.entry_bytes)? {
                Read::End => return Ok(()),
                Read::Incomplete => return Err(corrupt("live state journal has incomplete tail")),
                Read::Value(entry) => {
                    self.history.check(&entry.position.history)?;
                    if !visit(entry)? {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn header(reader: &mut BufReader<File>, history: &StoreHistory) -> Result<Header> {
    let Read::Value(header) = format::read::<Header>(reader, HEADER_BYTES)? else {
        return Err(corrupt("state journal header is incomplete"));
    };
    if header.version != VERSION {
        return Err(Error::backend(
            "open state root",
            BackendFailure::Unsupported,
            std::io::Error::new(std::io::ErrorKind::InvalidData, "state root uses a retired file format; open a fresh state root"),
        ));
    }
    history.check(&header.after.history)?;
    Ok(header)
}

fn conflict(reason: &'static str) -> Error {
    Error::backend("write state file", BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidData, reason))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_rollback_requires_reopen() {
        let root = tempfile::tempdir().unwrap();
        let history = history();
        let mut journal = StateLog::open(root.path(), history.clone(), settings()).unwrap();
        // A Read-Only Handle Forces Both Append And Rollback To Fail.
        journal.file = File::open(root.path().join("state.jsonl")).unwrap();
        let entry = RecoveryEntry {
            position: RecoveryPosition { history: history.clone(), sequence: 1 },
            required_data: crate::storage::StorageCursor { history: history.clone(), sequence: 0 },
            payload: vec![1],
        };
        assert!(journal.append(std::slice::from_ref(&entry)).is_err());
        assert!(journal.check().is_err());
        drop(journal);
        let mut reopened = StateLog::open(root.path(), history, settings()).unwrap();
        assert_eq!(reopened.position().sequence, 0);
        reopened.append(&[entry]).unwrap();
        assert_eq!(reopened.position().sequence, 1);
    }

    fn history() -> StoreHistory {
        StoreHistory { store: crate::Identifier::new("state.test").unwrap(), epoch: crate::Identifier::new("epoch.one").unwrap() }
    }

    fn settings() -> StorageStateSettings {
        StorageStateSettings {
            max_entry_bytes: std::num::NonZeroUsize::new(4096).unwrap(),
            max_checkpoint_bytes: std::num::NonZeroUsize::new(4096).unwrap(),
        }
    }
}
