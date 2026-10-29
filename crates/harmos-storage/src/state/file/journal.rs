/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Replay And Checkpoint Compaction

use std::fs::File;
use std::io::{BufReader, Read as _, Seek, SeekFrom, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::files::{Files, JournalOpener};
use super::format::{self, Read, corrupt, invalid, io};
use crate::storage::state::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StorageStateSettings};
use crate::storage::{ReadLimits, RecoveryPosition, StoreHistory};
use crate::{BackendFailure, Error, Result};

// The Development Format Has One Baseline.
const VERSION: u32 = 1;
// Headers Hold Only A Position, So Their Bound Is Harmos-Owned.
const HEADER_BYTES: usize = 4096;

pub(super) struct StateLog {
    files: Files,
    file: File,
    offsets: File,
    first: u64,
    #[cfg(test)]
    decoded: usize,
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

impl StateLog {
    #[tracing::instrument(name = "storage.state.file.open", level = "debug", skip_all)]
    pub(super) fn open(
        root: &Path, history: StoreHistory, settings: StorageStateSettings, opener: JournalOpener, check: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        let entry_bytes = settings.max_entry_bytes.get();
        let checkpoint_bytes = settings.max_checkpoint_bytes.get();
        let files = Files::open(root, opener)?;

        // The Journal Header Identifies The Format Before Any Payload Is Decoded.
        let mut file = files.journal()?;
        let created = file.metadata().map_err(io)?.len() == 0;
        if created {
            let header = Header {
                version: VERSION,
                after: RecoveryPosition { history: history.clone(), sequence: 0 },
            };
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
        let mut offsets = tempfile::tempfile().map_err(io)?;
        let first = header.after.sequence;
        let mut saved = first;
        loop {
            check()?;
            if saved > first && (saved - first) % 16_384 == 0 {
                tracing::debug!(sequence = saved, "State Journal Index Progress");
            }
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
                    offsets.write_all(&start.to_le_bytes()).map_err(io)?;
                    saved = entry.position.sequence;
                }
            }
        }
        if saved < base {
            return Err(corrupt("state journal does not reach checkpoint"));
        }
        Ok(Self {
            files,
            file,
            offsets,
            first,
            #[cfg(test)]
            decoded: 0,
            poisoned: false,
            history,
            saved,
            base,
            entry_bytes,
            checkpoint_bytes,
        })
    }

    #[tracing::instrument(name = "storage.state.file.check", level = "debug", skip_all)]
    pub(super) fn check(&self) -> Result<()> {
        if self.poisoned {
            return Err(io(std::io::Error::other("state journal requires close and reopen after an unresolved I/O failure")));
        }
        Ok(())
    }

    #[tracing::instrument(name = "storage.state.file.position", level = "debug", skip_all)]
    pub(super) fn position(&self) -> RecoveryPosition {
        RecoveryPosition {
            history: self.history.clone(),
            sequence: self.saved,
        }
    }

    #[tracing::instrument(name = "storage.state.file.snapshot", level = "debug", skip_all)]
    pub(super) fn snapshot(&self, maximum: usize) -> Result<Option<RecoverySnapshot>> {
        let bound = maximum.saturating_add(format::OVERHEAD);
        self.files.snapshot(bound.min(self.checkpoint_bytes))
    }

    #[tracing::instrument(name = "storage.state.file.page", level = "debug", skip_all)]
    pub(super) fn page(&mut self, after: &RecoveryPosition, through: &RecoveryPosition, limits: ReadLimits) -> Result<RecoveryPage> {
        self.history.check(&after.history)?;
        self.history.check(&through.history)?;
        if after.sequence < self.base {
            return Err(Error::Gap {
                requested: after.sequence,
                available: self.base,
            });
        }
        if after.sequence > through.sequence || through.sequence > self.saved {
            return Err(invalid("state read range is outside retained history"));
        }
        let mut entries = Vec::new();
        let mut bytes = 0usize;
        let mut last = after.sequence;

        let mut reader = self.reader_after(after.sequence)?;
        loop {
            if last == through.sequence {
                break;
            }
            #[cfg(test)]
            {
                self.decoded += 1;
            }
            let entry = match format::read::<RecoveryEntry>(&mut reader, self.entry_bytes)? {
                Read::End => break,
                Read::Incomplete => return Err(corrupt("live state journal has incomplete tail")),
                Read::Value(entry) => entry,
            };
            self.history.check(&entry.position.history)?;
            if entry.position.sequence <= after.sequence {
                continue;
            }
            if entry.position.sequence > through.sequence {
                break;
            }
            let size = crate::storage::manager::encoded_size(&entry)?;
            if entries.len() == limits.rows.get() || size > limits.bytes.get().saturating_sub(bytes) {
                if entries.is_empty() {
                    return Err(invalid("state entry exceeds requested page size"));
                }
                break;
            }
            entries.try_reserve(1).map_err(|_| invalid("state page allocation failed"))?;
            bytes += size;
            last = entry.position.sequence;
            entries.push(entry);
        }
        let next = (last < through.sequence).then(|| RecoveryPosition {
            history: self.history.clone(),
            sequence: last,
        });
        let page = RecoveryPage { entries, next };
        page.check(after, through, limits, |entry| crate::storage::manager::encoded_size(entry).unwrap_or(usize::MAX))?;
        Ok(page)
    }

    #[tracing::instrument(name = "storage.state.file.append", level = "debug", skip_all)]
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
                self.scan_after(sequence - 1, |stored| {
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
        // The Rebuildable Disk Index Never Retains One Heap Entry Per Journal Row.
        self.poisoned = true;
        self.offsets.seek(SeekFrom::End(0)).map_err(io)?;
        let mut offset = start;
        for line in &lines {
            self.offsets.write_all(&offset.to_le_bytes()).map_err(io)?;
            offset = offset.checked_add(line.len() as u64).ok_or_else(|| invalid("state journal offset overflow"))?;
        }
        self.saved = next;
        self.poisoned = false;
        Ok(())
    }

    #[tracing::instrument(name = "storage.state.file.flush", level = "debug", skip_all)]
    pub(super) fn flush(&mut self, through: &RecoveryPosition) -> Result<RecoveryPosition> {
        self.history.check(&through.history)?;
        if through.sequence > self.saved {
            return Err(invalid("state flush exceeds stored position"));
        }
        self.file.sync_all().map_err(io)?;
        Ok(self.position())
    }

    #[tracing::instrument(name = "storage.state.file.checkpoint", level = "debug", skip_all)]
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
            format::write(
                writer,
                &Header {
                    version: VERSION,
                    after: snapshot.through.clone(),
                },
                HEADER_BYTES,
            )?;
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
        let mut reader = self.reader()?;
        let header = {
            reader.seek(SeekFrom::Start(0)).map_err(io)?;
            header(&mut reader, &self.history)?
        };
        let mut offsets = tempfile::tempfile().map_err(io)?;
        loop {
            let offset = reader.stream_position().map_err(io)?;
            match format::read::<RecoveryEntry>(&mut reader, self.entry_bytes)? {
                Read::End => break,
                Read::Incomplete => return Err(corrupt("replaced state journal has incomplete tail")),
                Read::Value(_) => offsets.write_all(&offset.to_le_bytes()).map_err(io)?,
            }
        }
        self.first = header.after.sequence;
        self.offsets = offsets;
        self.poisoned = false;
        replaced
    }

    #[tracing::instrument(name = "storage.state.file.reader", level = "debug", skip_all)]
    fn reader(&self) -> Result<BufReader<File>> {
        let mut reader = BufReader::new(self.file.try_clone().map_err(io)?);
        reader.seek(SeekFrom::Start(0)).map_err(io)?;
        header(&mut reader, &self.history)?;
        Ok(reader)
    }

    // Sequence Offsets Are Rebuilt Once At Open And Replaced With Compaction.
    fn reader_after(&mut self, after: u64) -> Result<BufReader<File>> {
        let offset = if after >= self.saved {
            self.file.metadata().map_err(io)?.len()
        } else {
            let index = after
                .checked_sub(self.first)
                .and_then(|index| index.checked_mul(8))
                .ok_or_else(|| invalid("state journal index overflow"))?;
            self.offsets.seek(SeekFrom::Start(index)).map_err(io)?;
            let mut bytes = [0; 8];
            self.offsets.read_exact(&mut bytes).map_err(io)?;
            u64::from_le_bytes(bytes)
        };
        let mut reader = BufReader::with_capacity(256 * 1024, self.file.try_clone().map_err(io)?);
        reader.seek(SeekFrom::Start(offset)).map_err(io)?;
        Ok(reader)
    }

    #[tracing::instrument(name = "storage.state.file.scan", level = "debug", skip_all)]
    fn scan_after(&mut self, after: u64, mut visit: impl FnMut(RecoveryEntry) -> Result<bool>) -> Result<()> {
        let mut reader = self.reader_after(after)?;
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

fn header(reader: &mut BufReader<File>, history: &StoreHistory) -> Result<Header> {
    let Read::Value(header) = format::read::<Header>(reader, HEADER_BYTES)? else {
        return Err(corrupt("state journal header is incomplete"));
    };
    if header.version != VERSION {
        return Err(Error::backend(
            "open state root",
            BackendFailure::Unsupported,
            std::io::Error::new(std::io::ErrorKind::InvalidData, "unsupported state format; open a fresh state root"),
        ));
    }
    history.check(&header.after.history)?;
    Ok(header)
}

fn conflict(reason: &'static str) -> Error {
    Error::backend("write state file", BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidData, reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, StorageCursor};
    use std::{fs::OpenOptions, num::NonZeroUsize, sync::Arc};

    fn settings() -> StorageStateSettings {
        StorageStateSettings {
            max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
            max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
        }
    }

    fn open(path: &Path, history: &StoreHistory) -> StateLog {
        StateLog::open(
            path,
            history.clone(),
            settings(),
            Arc::new(|path| OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)),
            &mut || Ok(()),
        )
        .unwrap()
    }

    #[test]
    fn seven_day_recovery_seeks_and_compacts_without_rescanning_prefixes() {
        let root = tempfile::tempdir().unwrap();
        let history = StoreHistory {
            store: Identifier::new("journal.test").unwrap(),
            epoch: Identifier::new("one").unwrap(),
        };
        let mut journal = open(root.path(), &history);
        let per_day = 24 * 60;
        for day in 0..7 {
            let entries = (0..per_day)
                .map(|minute| {
                    let sequence = (day * per_day + minute + 1) as u64;
                    RecoveryEntry {
                        position: RecoveryPosition { history: history.clone(), sequence },
                        required_data: StorageCursor { history: history.clone(), sequence: 0 },
                        payload: sequence.to_le_bytes().to_vec(),
                    }
                })
                .collect::<Vec<_>>();
            journal.append(&entries).unwrap();
        }
        let through = journal.position();
        drop(journal);
        let mut journal = open(root.path(), &history);
        assert_eq!(journal.offsets.metadata().unwrap().len(), through.sequence * 8);
        let limits = ReadLimits {
            rows: NonZeroUsize::new(31).unwrap(),
            bytes: NonZeroUsize::new(16 * 1024).unwrap(),
        };
        let mut after = RecoveryPosition { history: history.clone(), sequence: 0 };
        let mut pages = 0;
        while after.sequence < through.sequence {
            let page = journal.page(&after, &through, limits).unwrap();
            for (index, entry) in page.entries.iter().enumerate() {
                assert_eq!(entry.position.sequence, after.sequence + index as u64 + 1);
                assert_eq!(entry.payload, entry.position.sequence.to_le_bytes());
            }
            after = page.entries.last().unwrap().position.clone();
            pages += 1;
        }
        assert!(journal.decoded <= through.sequence as usize + pages);
        // Arbitrary Retry Pages Also Seek Directly Rather Than Replaying History.
        journal.decoded = 0;
        let retry_after = RecoveryPosition {
            history: history.clone(),
            sequence: through.sequence - 3,
        };
        assert_eq!(journal.page(&retry_after, &through, limits).unwrap().entries.len(), 3);
        assert_eq!(journal.decoded, 3);
        let old_journal = std::fs::read(root.path().join("state.jsonl")).unwrap();
        let checkpoint = RecoveryPosition {
            history: history.clone(),
            sequence: through.sequence - 10,
        };
        let snapshot = RecoverySnapshot {
            through: checkpoint.clone(),
            required_data: StorageCursor { history: history.clone(), sequence: 0 },
            payload: vec![7],
        };
        journal.checkpoint(&snapshot).unwrap();
        assert_eq!(journal.offsets.metadata().unwrap().len(), 10 * 8);
        assert_eq!(journal.page(&checkpoint, &through, limits).unwrap().entries.len(), 10);
        drop(journal);
        // A Crash After Snapshot Publication But Before Journal Replacement.
        std::fs::write(root.path().join("state.jsonl"), old_journal).unwrap();
        let mut journal = open(root.path(), &history);
        assert_eq!(journal.snapshot(4096).unwrap(), Some(snapshot));
        assert_eq!(journal.page(&checkpoint, &through, limits).unwrap().entries.len(), 10);
        assert_eq!(journal.decoded, 10);
        assert!(matches!(journal.page(&retry_after, &checkpoint, limits), Err(Error::InvalidInput { .. })));
    }
    #[test]
    fn index_open_cancellation_preserves_the_durable_journal() {
        let root = tempfile::tempdir().unwrap();
        let history = StoreHistory {
            store: Identifier::new("cancel.test").unwrap(),
            epoch: Identifier::new("one").unwrap(),
        };
        let mut journal = open(root.path(), &history);
        let entries = (1..=8)
            .map(|sequence| RecoveryEntry {
                position: RecoveryPosition { history: history.clone(), sequence },
                required_data: StorageCursor { history: history.clone(), sequence: 0 },
                payload: vec![sequence as u8],
            })
            .collect::<Vec<_>>();
        journal.append(&entries).unwrap();
        drop(journal);
        let durable = std::fs::read(root.path().join("state.jsonl")).unwrap();
        let mut polls = 0;
        let cancelled = StateLog::open(
            root.path(),
            history.clone(),
            settings(),
            Arc::new(|path| OpenOptions::new().read(true).write(true).open(path)),
            &mut || {
                polls += 1;
                if polls == 3 { Err(Error::Closing { domain: "test cancellation" }) } else { Ok(()) }
            },
        );
        assert!(matches!(cancelled, Err(Error::Closing { .. })));
        assert_eq!(std::fs::read(root.path().join("state.jsonl")).unwrap(), durable);
        assert_eq!(open(root.path(), &history).position().sequence, 8);
    }
}
