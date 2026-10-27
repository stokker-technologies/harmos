/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Durable Segments With Streaming Replay And Cursor Pins.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::{config::JsonStorageSettings, format};
use crate::{
    BackendSnapshot, FileReference, MemorySize, ObjectEntry, Result, ScanPage, StorageBatch, StorageCursor, StorageSchema, StoreHistory,
    StoredIdentity, StoredRecord, StoredRow,
};

/* === Constants === */

const READ_BUFFER: usize = 256 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Log {
    pub root: PathBuf,
    pub history: StoreHistory,
    pub maximum: usize,
    pub saved: u64,
    segments: Vec<Segment>,
    pub snapshots: BTreeMap<Vec<u8>, (u64, u64)>,
    pub generation: u64,
    pub readers: BTreeMap<Vec<u8>, super::files::Reader>,
    pub pruned: BTreeMap<u64, u64>,
    pub pruned_through: u64,
    pub identities: BTreeMap<String, StoredIdentity>,
    last_identity: Option<StoredIdentity>,
    pub lease: Arc<File>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
struct Header {
    kind: String,
    first: StorageCursor,
    through: u64,
    count: usize,
}

struct Segment {
    header: Header,
    path: PathBuf,
}

/* === Enums === */

/* === Traits === */

pub(super) trait Row: MemorySize + Sized + Send + Sync + PartialEq + 'static {
    const KIND: &'static str;
    fn position(&self) -> &StorageCursor;
    fn encode(&self, writer: &mut dyn Write) -> Result<()>;
    fn decode(bytes: &[u8]) -> Result<Self>;
    fn identity(&self) -> Option<StoredIdentity> {
        None
    }
    fn same_identity(&self, _identity: &StoredIdentity) -> bool {
        false
    }
}

/* === Implementations === */

impl Log {
    pub fn open(settings: &JsonStorageSettings, history: StoreHistory) -> Result<Self> {
        format::regular(&settings.root)?;
        fs::create_dir_all(&settings.root).map_err(format::io)?;
        let lease_path = settings.root.join("json.lock");
        format::regular(&lease_path)?;
        let lease = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(lease_path).map_err(format::io)?;
        lease.try_lock().map_err(|_| format::conflict("storage root is already open"))?;
        for name in ["data", "staging", "files", "productions", "mutations", "releases"] {
            let path = settings.root.join(name);
            format::regular(&path)?;
            fs::create_dir_all(path).map_err(format::io)?;
        }
        let maximum = settings.max_entry_bytes.get();
        let identity = settings.root.join("history.json");
        match format::load::<StoreHistory>(&identity, maximum)? {
            Some(stored) => history.check(&stored)?,
            None => format::store(&identity, &history, maximum)?,
        }
        let mut segments = Vec::new();
        let mut identities = BTreeMap::new();
        let mut last_identity = None;
        for entry in fs::read_dir(settings.root.join("data")).map_err(format::io)? {
            let path = entry.map_err(format::io)?.path();
            format::regular(&path)?;
            if path.extension().is_none_or(|extension| extension != "jsonl") {
                continue;
            }
            let mut reader = BufReader::with_capacity(READ_BUFFER, File::open(&path).map_err(format::io)?);
            let line = format::line(&mut reader, maximum)?.ok_or_else(|| format::corrupt("empty committed segment"))?;

            let header: Header = serde_json::from_slice(&line).map_err(format::json)?;

            drop(line);
            history.check(&header.first.history)?;
            if header.first.sequence == 0
                || header.count == 0
                || header.through.checked_sub(header.first.sequence) != Some(header.count as u64 - 1)
            {
                return Err(format::corrupt("invalid segment range"));
            }
            let mut sequence = header.first.sequence;
            while let Some(bytes) = format::line(&mut reader, maximum)? {
                let (position, identity) = position(&header.kind, &bytes, last_identity.as_ref())?;
                if let Some(identity) = identity {
                    remember(&mut identities, identity.clone())?;
                    last_identity = Some(identity);
                }
                history.check(&position.history)?;
                if position.sequence != sequence {
                    return Err(format::corrupt("segment positions are not contiguous"));
                }
                sequence = sequence.checked_add(1).ok_or_else(|| format::corrupt("segment position overflow"))?;
            }
            if sequence - 1 != header.through {
                return Err(format::corrupt("segment row count differs"));
            }
            segments.try_reserve_exact(1).map_err(|_| exhausted())?;
            segments.push(Segment { header, path });
        }
        segments.sort_by_key(|segment| segment.header.first.sequence);
        let mut saved = 0_u64;
        for segment in &segments {
            if segment.header.first.sequence != saved + 1 {
                return Err(format::corrupt("committed storage has a cursor gap"));
            }
            saved = segment.header.through;
        }
        let pruning = format::load::<super::reads::Pruning>(&settings.root.join("pruned.json"), maximum)?;
        let pruned_through = pruning.as_ref().map_or(0, |pruning| pruning.through);
        let pruned = pruning.map(|pruning| pruning.positions).unwrap_or_default();
        let generation = pruned.values().copied().max().unwrap_or(0);
        Ok(Self {
            root: settings.root.clone(),
            history,
            maximum,
            saved,
            segments,
            snapshots: BTreeMap::new(),
            generation,
            readers: BTreeMap::new(),
            pruned,
            pruned_through,
            identities,
            last_identity,
            lease: Arc::new(lease),
        })
    }

    pub fn cursor(&self) -> StorageCursor {
        StorageCursor {
            history: self.history.clone(),
            sequence: self.saved,
        }
    }

    pub fn snapshot(&mut self, at: StorageCursor) -> Result<BackendSnapshot> {
        self.history.check(&at.history)?;
        if at.sequence > self.saved {
            return Err(format::invalid("snapshot exceeds saved frontier"));
        }
        if at.sequence < self.pruned_through {
            return Err(crate::Error::Gap {
                requested: at.sequence,
                available: self.pruned_through,
            });
        }
        let mut token = vec![0; 16];
        getrandom::fill(&mut token)
            .map_err(|error| crate::Error::backend("JSON snapshot entropy", crate::BackendFailure::Permanent, error))?;
        self.snapshots.insert(token.clone(), (at.sequence, self.generation));
        Ok(BackendSnapshot { at, token })
    }

    pub fn pin(&self, snapshot: &BackendSnapshot) -> Result<u64> {
        self.history.check(&snapshot.at.history)?;
        self.snapshots
            .get(&snapshot.token)
            .filter(|pin| pin.0 == snapshot.at.sequence)
            .map(|pin| pin.1)
            .ok_or_else(|| format::invalid("snapshot is released or unknown"))
    }

    pub fn write<T: Row>(&mut self, batch: StorageBatch<T>) -> Result<StorageCursor> {
        let rows = batch.as_slice();
        if rows.is_empty() {
            return Err(format::invalid("typed batch is empty"));
        }
        let first = rows[0].position();
        self.history.check(&first.history)?;
        for (index, row) in rows.iter().enumerate() {
            self.history.check(&row.position().history)?;
            if first.sequence.checked_add(index as u64) != Some(row.position().sequence) {
                return Err(format::invalid("typed batch positions are not contiguous"));
            }
        }
        if first.sequence == 0 || first.sequence > self.saved + 1 {
            return Err(format::conflict("typed write skips the saved frontier"));
        }
        let overlap = rows.partition_point(|row| row.position().sequence <= self.saved);
        for row in &rows[..overlap] {
            let stored = self.one::<T>(row.position().sequence)?;
            if stored.as_ref() != Some(row) {
                return Err(format::conflict("typed retry differs from durable bytes"));
            }
        }
        if overlap == rows.len() {
            return Ok(self.cursor());
        }
        let rows = &rows[overlap..];
        let header = Header {
            kind: T::KIND.into(),
            first: rows[0].position().clone(),
            through: rows.last().expect("nonempty suffix").position().sequence,
            count: rows.len(),
        };
        let name = format!("{:020}-{:020}-{}.jsonl", header.first.sequence, header.through, T::KIND);
        let path = self.root.join("data").join(name);
        let pending = path.with_extension("pending");
        format::regular(&path)?;
        format::regular(&pending)?;
        let mut writer = BufWriter::new(File::create(&pending).map_err(format::io)?);
        format::write(&mut writer, self.maximum, |writer| format::encode(&header, writer))?;
        for row in rows {
            format::write(&mut writer, self.maximum, |writer| row.encode(writer))?;
        }
        writer.flush().map_err(format::io)?;
        writer.get_ref().sync_all().map_err(format::io)?;
        drop(writer);
        if path.exists() {
            let mut reader = BufReader::with_capacity(READ_BUFFER, File::open(&path).map_err(format::io)?);
            let prior = format::line(&mut reader, self.maximum)?.ok_or_else(|| format::corrupt("missing uncertain commit header"))?;

            let prior: Header = serde_json::from_slice(&prior).map_err(format::json)?;
            if prior != header {
                return Err(format::conflict("uncertain typed commit range differs"));
            }
            for row in rows {
                let line = format::line(&mut reader, self.maximum)?.ok_or_else(|| format::corrupt("incomplete uncertain commit"))?;

                if T::decode(&line)? != *row {
                    return Err(format::conflict("uncertain typed commit differs"));
                }
            }
            if format::line(&mut reader, self.maximum)?.is_some() {
                return Err(format::conflict("uncertain commit has extra rows"));
            }
            fs::remove_file(pending).map_err(format::io)?;
        } else {
            fs::rename(pending, &path).map_err(format::io)?;
        }
        format::sync_directory(&self.root.join("data"))?;
        self.saved = header.through;
        for row in rows {
            if self.last_identity.as_ref().is_some_and(|identity| row.same_identity(identity)) {
                continue;
            }
            if let Some(identity) = row.identity() {
                remember(&mut self.identities, identity.clone())?;
                self.last_identity = Some(identity);
            }
        }
        self.segments.push(Segment { header, path });
        Ok(self.cursor())
    }

    pub(super) fn visit_sized<T: Row>(&self, at: u64, after: u64, mut visitor: impl FnMut(T, usize) -> Result<bool>) -> Result<()> {
        for segment in &self.segments {
            if segment.header.kind != T::KIND || segment.header.through <= after || segment.header.first.sequence > at {
                continue;
            }
            let mut reader = BufReader::with_capacity(READ_BUFFER, File::open(&segment.path).map_err(format::io)?);
            let header = format::line(&mut reader, self.maximum)?.ok_or_else(|| format::corrupt("segment header disappeared"))?;
            drop(header);

            while let Some(line) = format::line(&mut reader, self.maximum)? {
                let row = T::decode(&line)?;

                let sequence = row.position().sequence;
                if sequence > at {
                    break;
                }
                if sequence > after && !visitor(row, line.len())? {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub fn scan<T: Row>(&self, at: u64, generation: u64, page: ScanPage, mut select: impl FnMut(&T) -> Result<bool>) -> Result<Vec<T>> {
        self.scan_read(at, generation, page, |row| select(row))
    }

    pub(super) fn scan_read<T: Row>(
        &self, at: u64, generation: u64, page: ScanPage, mut select: impl FnMut(&T) -> Result<bool>,
    ) -> Result<Vec<T>> {
        let mut output = Vec::new();
        let mut bytes = 0usize;
        self.visit_sized::<T>(at, page.after, |row, size| {
            let sequence = row.position().sequence;
            if self.pruned.get(&sequence).is_some_and(|pruned| *pruned <= generation) || !select(&row)? {
                return Ok(true);
            }
            if !output.is_empty() && size > page.bytes.get().saturating_sub(bytes) {
                return Ok(false);
            }
            reserve(&mut output, page.rows.get())?;
            bytes = add(bytes, size)?;
            output.push(row);
            Ok(output.len() < page.rows.get() && bytes < page.bytes.get())
        })?;
        Ok(output)
    }

    pub fn one<T: Row>(&self, sequence: u64) -> Result<Option<T>> {
        let mut found = None;
        self.visit_sized::<T>(sequence, sequence.saturating_sub(1), |row, _| {
            found = Some(row);
            Ok(false)
        })?;
        Ok(found)
    }
}

impl Row for StorageSchema {
    const KIND: &'static str = "schemas";
    fn position(&self) -> &StorageCursor {
        &self.position
    }
    fn encode(&self, writer: &mut dyn Write) -> Result<()> {
        format::encode(self, writer)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(format::json)
    }
}

impl Row for StoredRecord {
    const KIND: &'static str = "records";
    fn position(&self) -> &StorageCursor {
        &self.position
    }
    fn encode(&self, writer: &mut dyn Write) -> Result<()> {
        format::encode(self, writer)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(format::json)
    }
}

impl Row for StoredRow {
    const KIND: &'static str = "stream";
    fn position(&self) -> &StorageCursor {
        &self.record.position
    }
    fn encode(&self, writer: &mut dyn Write) -> Result<()> {
        format::encode(self, writer)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(format::json)
    }
    fn identity(&self) -> Option<StoredIdentity> {
        Some(StoredIdentity::Stream {
            schema: self.record.schema.clone(),
            scope: self.record.scope.clone(),
            item: self.record.item.clone(),
            stream: self.stream.clone(),
            first: self.record.position.clone(),
        })
    }
    fn same_identity(&self, identity: &StoredIdentity) -> bool {
        matches!(identity, StoredIdentity::Stream {schema,scope,item,stream,..} if schema == &self.record.schema && scope == &self.record.scope && item == &self.record.item && stream == &self.stream)
    }
}

impl Row for ObjectEntry {
    const KIND: &'static str = "objects";
    fn position(&self) -> &StorageCursor {
        &self.position
    }
    fn encode(&self, writer: &mut dyn Write) -> Result<()> {
        format::encode(self, writer)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(format::json)
    }
    fn identity(&self) -> Option<StoredIdentity> {
        Some(StoredIdentity::Object {
            schema: self.value.schema.clone(),
            key: self.value.item.clone(),
            first: self.position.clone(),
        })
    }
    fn same_identity(&self, identity: &StoredIdentity) -> bool {
        matches!(identity, StoredIdentity::Object {schema,key,..} if schema == &self.value.schema && key == &self.value.item)
    }
}

impl Row for FileReference {
    const KIND: &'static str = "files";
    fn position(&self) -> &StorageCursor {
        &self.position
    }
    fn encode(&self, writer: &mut dyn Write) -> Result<()> {
        format::encode(self, writer)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(format::json)
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn exhausted() -> crate::Error {
    crate::Error::ResourceExhausted { resource: "JSON read memory" }
}

pub(super) fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or_else(exhausted)
}

pub(super) fn reserve<T>(output: &mut Vec<T>, limit: usize) -> Result<()> {
    if output.len() < output.capacity() {
        return Ok(());
    }
    let target = output.capacity().max(2).checked_mul(2).ok_or_else(exhausted)?.min(limit);

    output.try_reserve_exact(target - output.len()).map_err(|_| exhausted())?;
    Ok(())
}

fn position(kind: &str, bytes: &[u8], previous: Option<&StoredIdentity>) -> Result<(StorageCursor, Option<StoredIdentity>)> {
    match kind {
        "schemas" => {
            let row = StorageSchema::decode(bytes)?;
            Ok((row.position, None))
        }
        "objects" => {
            let row = ObjectEntry::decode(bytes)?;
            let identity =
                if previous.is_some_and(|identity| identity.first().sequence <= row.position.sequence && row.same_identity(identity)) {
                    None
                } else {
                    row.identity()
                };
            Ok((row.position, identity))
        }
        "records" => {
            let row = StoredRecord::decode(bytes)?;
            Ok((row.position, None))
        }
        "stream" => {
            let row = StoredRow::decode(bytes)?;
            let identity = if previous
                .is_some_and(|identity| identity.first().sequence <= row.record.position.sequence && row.same_identity(identity))
            {
                None
            } else {
                row.identity()
            };
            Ok((row.record.position, identity))
        }
        "files" => {
            let row = FileReference::decode(bytes)?;
            Ok((row.position, None))
        }
        _ => Err(format::corrupt("unknown typed segment kind")),
    }
}

fn remember(identities: &mut BTreeMap<String, StoredIdentity>, identity: StoredIdentity) -> Result<()> {
    let key = match &identity {
        StoredIdentity::Stream { schema, scope, item, stream, .. } => {
            serde_json::to_string(&(schema, scope, item, stream)).map_err(format::json)?
        }
        StoredIdentity::Object { schema, key, .. } => serde_json::to_string(&(schema, key)).map_err(format::json)?,
    };
    if identities.get(&key).is_none_or(|prior| prior.first().sequence > identity.first().sequence) {
        identities.insert(key, identity);
    }
    Ok(())
}

/* === Tests === */
