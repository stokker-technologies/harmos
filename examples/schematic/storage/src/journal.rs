/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod wire {
    use std::fs::File;
    use std::io::{self, BufRead, BufReader, Write};
    use std::path::Path;
    use std::time::{Duration, UNIX_EPOCH};

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Line<P> {
        pub(super) position: u64,
        pub(super) kind: Kind,
        pub(super) id: String,
        pub(super) version: u32,
        pub(super) metadata: Stamp,
        pub(super) payload: P,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    pub(super) enum Kind {
        Transaction,
        Record,
    }

    impl Kind {
        pub(super) fn of(kind: harmos::state::Kind) -> Self {
            match kind {
                harmos::state::Kind::Transaction => Self::Transaction,
                harmos::state::Kind::Record => Self::Record,
            }
        }

        pub(super) fn into_kind(self) -> harmos::state::Kind {
            match self {
                Self::Transaction => harmos::state::Kind::Transaction,
                Self::Record => harmos::state::Kind::Record,
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Stamp {
        pub(super) principal: Option<String>,
        pub(super) correlation: u64,
        pub(super) causation: Option<u64>,
        pub(super) request: Option<String>,
        pub(super) link: Option<Link>,
        pub(super) at_us: i64,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    pub(super) enum Link {
        UndoOf(u64),
        RedoOf(u64),
    }

    impl Link {
        fn of(link: harmos::state::Link) -> Self {
            match link {
                harmos::state::Link::UndoOf(position) => Self::UndoOf(position.0),
                harmos::state::Link::RedoOf(position) => Self::RedoOf(position.0),
            }
        }

        fn into_link(self) -> harmos::state::Link {
            match self {
                Self::UndoOf(position) => harmos::state::Link::UndoOf(harmos::state::Position(position)),
                Self::RedoOf(position) => harmos::state::Link::RedoOf(harmos::state::Position(position)),
            }
        }
    }

    impl Stamp {
        pub(super) fn witnessing(metadata: &harmos::state::Metadata) -> Self {
            let at_us = match metadata.at.duration_since(UNIX_EPOCH) {
                Ok(since) => i64::try_from(since.as_micros()).unwrap_or(i64::MAX),
                Err(before) => i64::try_from(before.duration().as_micros()).map_or(i64::MIN, |micros| -micros),
            };

            Self {
                principal: metadata.principal.as_ref().map(|principal| principal.as_str().to_owned()),
                correlation: metadata.correlation.0.0,
                causation: metadata.causation.map(|causation| causation.0.0),
                request: metadata.request.as_ref().map(|request| request.as_str().to_owned()),
                link: metadata.link.map(Link::of),
                at_us,
            }
        }

        pub(super) fn witnessed(self) -> Result<harmos::state::Metadata, serde_json::Error> {
            use serde::de::Error;

            let at = if self.at_us >= 0 {
                UNIX_EPOCH.checked_add(Duration::from_micros(self.at_us.unsigned_abs()))
            } else {
                UNIX_EPOCH.checked_sub(Duration::from_micros(self.at_us.unsigned_abs()))
            }
            .ok_or_else(|| serde_json::Error::custom("at_us lies outside this platform's clock"))?;

            let mut metadata = harmos::state::Metadata::deserialize(serde_json::json!({
                "principal": null,
                "correlation": 0,
                "causation": null,
                "request": null,
                "link": null,
                "at": { "secs_since_epoch": 0, "nanos_since_epoch": 0 },
            }))?;
            metadata.principal = self.principal.map(harmos::state::PrincipalId::new);
            metadata.correlation = harmos::state::CorrelationId(harmos::state::Position(self.correlation));
            metadata.causation = self.causation.map(|causation| harmos::state::CausationId(harmos::state::Position(causation)));
            metadata.request = self.request.map(harmos::state::RequestKey::new);
            metadata.link = self.link.map(Link::into_link);
            metadata.at = at;

            Ok(metadata)
        }
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Header {
        pub(super) schema: u32,
        pub(super) generation: u64,
        pub(super) origin: u64,
    }

    impl Header {
        pub(super) fn read(path: &Path) -> io::Result<Option<Self>> {
            let Ok(file) = File::open(path) else {
                return Ok(None);
            };
            use std::io::Read;
            let mut first = Vec::new();
            BufReader::new(file).take((super::MAX_HEADER_BYTES + 2) as u64).read_until(b'\n', &mut first)?;
            if first.is_empty() {
                return Ok(None);
            }
            if first.len() > super::MAX_HEADER_BYTES + 1 {
                return Err(io::Error::other("journal header exceeds byte limit"));
            }
            if first.last() == Some(&b'\n') {
                first.pop();
            }
            let first = lf_only(first)?;

            serde_json::from_str(&first).map(Some).map_err(io::Error::other)
        }

        pub(super) fn write(&self, file: &mut File) -> io::Result<()> {
            let header = serde_json::to_string(self).map_err(io::Error::other)?;

            writeln!(file, "{header}")
        }
    }

    pub(super) fn lf_only(line: Vec<u8>) -> io::Result<String> {
        if line.last() == Some(&b'\r') {
            return Err(io::Error::other("the wire is LF-only: a stored line ends with a carriage return"));
        }
        String::from_utf8(line).map_err(io::Error::other)
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
// Third Party Imports
use serde::{Deserialize, Deserializer, Serializer};
// Local Imports
use harmos::{
    Application, DefinitionId, Error, runtime::Incompatible, state::Entry, state::Envelope, state::Kind, state::Position, state::Recorded,
    state::SystemFact, storage::JournalConsumer, storage::Payload, storage::Recovery, storage::Source,
};

/* === Constants === */

const SCHEMA: u32 = 3;
const LEGACY_SCHEMA: u32 = 2;
const MAX_HEADER_BYTES: usize = 4096;
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Jsonl<A: Wire> {
    log: Arc<Mutex<Log>>,
    batch: usize,
    after: Option<Position>,
    forward: BTreeMap<Position, String>,
    halted: bool,
    application: PhantomData<fn() -> A>,
}

pub struct Sidecar {
    log: Arc<Mutex<Log>>,
}

struct Document {
    generation: u64,
    at: Position,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group<'a> {
    group: u32,
    first: u64,
    frontier: u64,

    #[serde(borrow)]
    entries: &'a serde_json::value::RawValue,
}

struct Members(usize);

struct Log {
    path: PathBuf,
    file: File,
    generation: u64,
    origin: Position,
    schema: u32,
    append_ready: bool,
    failed: bool,
    pending: Vec<String>,
    written: Position,
    checkpoint: Position,
}

/* === Enums === */

/* === Traits === */

pub trait Wire: Application + Sized {
    fn identify(value: &Recorded<Self>) -> (DefinitionId, u32);

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error>;

    fn decode<'de, D: Deserializer<'de> + Copy>(
        kind: Kind, id: &DefinitionId, version: u32, payload: D,
    ) -> Result<Option<Payload<Self>>, D::Error>;
}

/* === Implementations === */

impl<A: Wire> Jsonl<A> {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            log: Arc::new(Mutex::new(Log::open(path.as_ref(), None)?)),
            batch: 1,
            after: None,
            forward: BTreeMap::new(),
            halted: false,
            application: PhantomData,
        })
    }

    pub fn sidecar(path: impl AsRef<Path>, generation: u64, at: Position) -> io::Result<Self> {
        Ok(Self {
            log: Arc::new(Mutex::new(Log::open(path.as_ref(), Some(Document { generation, at }))?)),
            batch: 1,
            after: None,
            forward: BTreeMap::new(),
            halted: false,
            application: PhantomData,
        })
    }

    pub fn after(mut self, at: Position) -> Self {
        self.after = Some(at);
        self
    }

    pub fn syncing_every(mut self, entries: usize) -> Self {
        self.batch = entries.max(1);
        self
    }

    pub fn sidecars(&self) -> Sidecar {
        Sidecar { log: Arc::clone(&self.log) }
    }

    fn encode(&self, entry: &Entry<Recorded<A>>) -> Result<String, serde_json::Error> {
        let (id, version, payload) = if let Some(prepared) = &entry.prepared {
            (prepared.id.clone(), prepared.version, prepared.payload.clone())
        } else {
            match &entry.value {
                Recorded::System(fact) => {
                    let (id, version) = fact.identify();
                    let payload = serde::Serialize::serialize(&fact.payload, serde_json::value::Serializer)?;

                    (id, version, payload)
                }
                Recorded::Transaction(_) | Recorded::Record(_) => {
                    let (id, version) = A::identify(&entry.value);
                    let payload = A::encode(&entry.value, serde_json::value::Serializer)?;

                    (id, version, payload)
                }
            }
        };

        serde_json::to_string(&wire::Line {
            position: entry.position.0,
            kind: wire::Kind::of(entry.value.kind()),
            id: id.as_str().to_owned(),
            version,
            metadata: wire::Stamp::witnessing(&entry.metadata),
            payload,
        })
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        self.log.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<A: Wire> JournalConsumer<A> for Jsonl<A> {
    fn resume(&self) -> Position {
        self.log().written
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, _applied: Position) -> Position {
        // A halted copy refuses to advance: the stalled checkpoint is the
        // loud form of the gap it refused to write across.
        if self.halted {
            return self.log().checkpoint;
        }
        let mut frames = std::collections::BTreeMap::new();
        for (_, line) in std::mem::take(&mut self.forward) {
            let Ok(members) = members(&line) else {
                self.halted = true;
                return self.log().checkpoint;
            };
            let first = Position(members[0].position);
            let end = Position(members.last().expect("validated frame").position);
            frames.insert(first, (end, line));
        }
        let mut index = 0;
        while index < entries.len() {
            let entry = &entries[index];
            if frames.range(..=entry.position).next_back().is_some_and(|(_, (end, _))| *end >= entry.position) {
                index += 1;
                continue;
            }
            let (first, end) = entry.metadata.commit.unwrap_or((entry.position, entry.position));
            if first != entry.position {
                self.halted = true;
                return self.log().checkpoint;
            }
            let count = usize::try_from(end.0 - first.0 + 1).unwrap_or(usize::MAX);
            let Some(group) = entries.get(index..).and_then(|remaining| remaining.get(..count)) else {
                self.halted = true;
                return self.log().checkpoint;
            };
            if group
                .iter()
                .enumerate()
                .any(|(offset, member)| member.position.0 != first.0 + offset as u64 || member.metadata.commit != entry.metadata.commit)
            {
                self.halted = true;
                return self.log().checkpoint;
            }
            let encoded = group.iter().map(|member| self.encode(member)).collect::<Result<Vec<_>, _>>();
            let Ok(encoded) = encoded else {
                self.halted = true;
                return self.log().checkpoint;
            };
            let line = if entry.metadata.commit.is_some() {
                format!("{{\"group\":1,\"first\":{},\"frontier\":{},\"entries\":[{}]}}", first.0, end.0, encoded.join(","))
            } else {
                encoded.into_iter().next().expect("one standalone entry")
            };
            if line.len() > MAX_FRAME_BYTES {
                self.halted = true;
                return self.log().checkpoint;
            }
            frames.insert(first, (end, line));
            index += count;
        }
        let batch = self.batch;
        let mut log = self.log();
        let mut gapped = false;
        for (first, (end, line)) in frames {
            if Some(first.0) != log.written.0.checked_add(1) {
                gapped = true;
                break;
            }
            log.pending.push(line);
            log.written = end;
        }
        if log.pending.len() >= batch {
            log.persist();
        }
        let checkpoint = log.checkpoint;
        drop(log);
        if gapped {
            self.halted = true;
        }
        checkpoint
    }

    async fn flush(&mut self) {
        // The held batch is this copy's declared risk window; stop closes it,
        // so a clean stop loses nothing however lazily the copy was syncing.
        self.log().persist();
    }

    fn redelivered(&mut self, stored: &[(Position, Vec<u8>)]) {
        if self.halted {
            return;
        }
        for (position, bytes) in stored {
            // A forwarded line is written exactly as stored — verbatim, byte
            // for byte. Bytes that are not one line of JSON text cannot be a
            // line of this copy; such an entry falls back to the codec where
            // a decoded entry exists, and stays with the source otherwise.
            let Ok(line) = std::str::from_utf8(bytes) else {
                continue;
            };
            if serde_json::from_str::<serde_json::Value>(line).is_err() {
                continue;
            }
            self.forward.insert(*position, line.to_owned());
        }
    }
}

impl<A: Wire> Source<A> for Jsonl<A> {
    async fn load(&mut self) -> Result<Recovery<A>, Incompatible> {
        // Finite, once, and never a supervised work item: the whole copy is
        // read here, between freezing the catalog and opening the writer.
        let mut log = self.log();
        let stored = log.read().map_err(Incompatible::unreadable)?;
        let origin = self.after.unwrap_or(log.origin).max(log.origin);
        let mut entries = Vec::new();
        let mut encoded = Vec::new();
        let mut commits = Vec::new();

        for line in stored {
            let group = members(&line).map_err(Incompatible::unreadable)?;
            let first = Position(group[0].position);
            let end = Position(group.last().expect("validated group").position);
            if first <= origin && origin < end {
                return Err(Incompatible::unreadable("recovery origin splits a logical commit"));
            }
            if end <= origin {
                continue;
            }
            log.written = end;
            log.checkpoint = end;
            encoded.push((end, line.as_bytes().to_vec()));
            let grouped = is_group(&line);
            if grouped {
                commits.push((first, end));
            }
            for member in group {
                let mut envelope = migrated_member(member)?;
                if grouped {
                    envelope.metadata.commit = Some((first, end));
                }
                if let Some(recovered) = decoded::<A>(envelope)? {
                    entries.push(recovered);
                }
            }
        }
        log.written = log.written.max(origin);
        log.checkpoint = log.checkpoint.max(origin);

        Ok(Recovery { commits, frontier: log.written, entries, encoded })
    }

    async fn read(&mut self, after: Position, through: Position) -> Result<Recovery<A>, Error> {
        // The mutex pins the header horizon for the whole reread: this source's
        // prune cannot overtake a fold and turn it into a partial answer.
        let log = self.log();
        if after < log.origin {
            return Err(Error::BeneathHorizon);
        }
        if through > log.checkpoint {
            return Err(Error::BeyondTail);
        }
        let stored = log.read().map_err(|_| Error::Unavailable)?;
        let mut entries = Vec::new();
        let mut encoded = Vec::new();
        let mut commits = Vec::new();

        for line in stored {
            let group = members(&line).map_err(|_| Error::Unavailable)?;
            let first = Position(group[0].position);
            let end = Position(group.last().expect("validated group").position);
            if end <= after || first > through {
                continue;
            }
            if first > after && end <= through {
                encoded.push((end, line.as_bytes().to_vec()));
            }
            let grouped = is_group(&line);
            if grouped {
                commits.push((first, end));
            }
            for member in group {
                if member.position <= after.0 || member.position > through.0 {
                    continue;
                }
                let mut envelope = migrated_member(member).map_err(|_| Error::Unavailable)?;
                if grouped {
                    envelope.metadata.commit = Some((first, end));
                }
                if let Some(recovered) = decoded::<A>(envelope).map_err(|_| Error::Unavailable)? {
                    entries.push(recovered);
                }
            }
        }

        Ok(Recovery { commits, frontier: through, entries, encoded })
    }

    async fn prune(&mut self, beneath: Position) {
        self.log().prune(beneath).ok();
    }
}

impl Sidecar {
    pub fn save(&self, export: impl FnOnce(u64, Position) -> io::Result<()>) -> io::Result<u64> {
        let mut log = self.log.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = log.generation + 1;
        let at = log.written;

        export(generation, at)?;
        log.reset(generation, at)?;

        Ok(generation)
    }

    pub fn generation(&self) -> u64 {
        self.log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).generation
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for Members {
    type Value = Vec<wire::Line<serde_json::Value>>;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> serde::de::Visitor<'de> for Members {
    type Value = Vec<wire::Line<serde_json::Value>>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exactly {} commit members", self.0)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut members = Vec::with_capacity(self.0);
        for _ in 0..self.0 {
            let member = sequence.next_element()?.ok_or_else(|| serde::de::Error::custom("missing commit member"))?;
            members.push(member);
        }
        if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(serde::de::Error::custom("extra commit member"));
        }
        Ok(members)
    }
}

impl Log {
    fn open(path: &Path, document: Option<Document>) -> io::Result<Self> {
        let (mut log, schema) = match wire::Header::read(path)? {
            Some(header) => {
                if document.is_none() && !matches!(header.schema, SCHEMA | LEGACY_SCHEMA) {
                    return Err(io::Error::other(format!(
                        "the copy was written by schema {}, which this build cannot read",
                        header.schema
                    )));
                }
                (
                    Self {
                        path: path.to_owned(),
                        file: OpenOptions::new().append(true).open(path)?,
                        schema: header.schema,
                        append_ready: false,
                        failed: false,
                        generation: header.generation,
                        origin: Position(header.origin),
                        pending: Vec::new(),
                        written: Position(header.origin),
                        checkpoint: Position(header.origin),
                    },
                    header.schema,
                )
            }
            None => (Self::create(path, 0, Position::ORIGIN)?, SCHEMA),
        };
        // A retired-format sidecar is reset below without its lines ever
        // being scanned; only a current-format copy reads its last position.
        if matches!(schema, SCHEMA | LEGACY_SCHEMA) {
            log.written = log.last()?.max(log.origin);
            log.checkpoint = log.written;
        }

        // A sidecar folded from another save — or written by another format
        // version — is not this document's history.
        if let Some(document) = document
            && (log.generation != document.generation || !matches!(schema, SCHEMA | LEGACY_SCHEMA))
        {
            log.reset(document.generation, document.at)?;
        }

        Ok(log)
    }

    fn create(path: &Path, generation: u64, origin: Position) -> io::Result<Self> {
        let header = wire::Header { schema: SCHEMA, generation, origin: origin.0 };
        let mut file = File::create(path)?;
        header.write(&mut file)?;
        file.sync_all()?;

        Ok(Self {
            path: path.to_owned(),
            file: OpenOptions::new().append(true).open(path)?,
            generation,
            origin,
            schema: SCHEMA,
            append_ready: true,
            failed: false,
            pending: Vec::new(),
            written: origin,
            checkpoint: origin,
        })
    }

    fn read(&self) -> io::Result<Vec<String>> {
        self.scan().map(|(lines, _)| lines)
    }

    fn scan(&self) -> io::Result<(Vec<String>, u64)> {
        let mut reader = BufReader::new(File::open(&self.path)?);
        let mut line = Vec::new();
        {
            use std::io::Read;
            (&mut reader).take((MAX_HEADER_BYTES + 2) as u64).read_until(b'\n', &mut line)?;
            if line.len() > MAX_HEADER_BYTES + 1 {
                return Err(io::Error::other("journal header exceeds byte limit"));
            }
        }
        let mut valid = line.len() as u64;
        let mut result = Vec::new();
        let mut expected = self.origin.0.checked_add(1);
        loop {
            line.clear();
            // Take bounds allocation even for a file with no terminator.
            use std::io::Read;
            (&mut reader).take((MAX_FRAME_BYTES + 2) as u64).read_until(b'\n', &mut line)?;
            if line.is_empty() {
                break;
            }
            if line.len() > MAX_FRAME_BYTES + 1 {
                return Err(io::Error::other("journal frame exceeds byte limit"));
            }
            let terminated = line.last() == Some(&b'\n');
            let prefix = b"{\"group\":";
            if !terminated && self.schema == SCHEMA && (line.starts_with(prefix) || prefix.starts_with(&line)) {
                break;
            }
            if terminated {
                line.pop();
            }
            let text = wire::lf_only(line.clone())?;
            let group = members(&text)?;
            if self.schema == LEGACY_SCHEMA && is_group(&text) {
                return Err(io::Error::other("v2 cannot contain group frames"));
            }
            if Some(group[0].position) != expected {
                return Err(io::Error::other("journal position gap"));
            }
            expected = group.last().expect("validated group").position.checked_add(1);
            valid += line.len() as u64 + u64::from(terminated);
            result.push(text);
        }
        Ok((result, valid))
    }

    fn last(&self) -> io::Result<Position> {
        let mut last = self.origin;
        for line in self.read()? {
            last = Position(members(&line)?.last().expect("validated group").position);
        }
        Ok(last)
    }

    fn prepare_append(&mut self) -> io::Result<()> {
        if self.append_ready {
            return Ok(());
        }
        let (lines, valid) = self.scan()?;
        if self.schema == SCHEMA {
            self.truncate(valid)?;
            use std::io::{Read, Seek};
            if valid > 0 {
                let mut last = File::open(&self.path)?;
                last.seek(std::io::SeekFrom::Start(valid - 1))?;
                let mut byte = [0];
                last.read_exact(&mut byte)?;
                if byte[0] != b'\n' {
                    self.file.write_all(b"\n")?;
                }
            }
            self.append_ready = true;
            return Ok(());
        }
        let temporary = self.path.with_extension("upgrade~");
        let mut fresh = File::create(&temporary)?;
        wire::Header { schema: SCHEMA, generation: self.generation, origin: self.origin.0 }.write(&mut fresh)?;
        for line in lines {
            writeln!(fresh, "{line}")?;
        }
        fresh.sync_all()?;
        std::fs::rename(&temporary, &self.path)?;
        // Directory fsync is a POSIX idiom, as it is for the evidence
        // directory beside this module: opening a directory as a file is
        // refused on Windows (`ERROR_ACCESS_DENIED`), and NTFS makes rename
        // metadata durable without a user-mode directory flush.
        if cfg!(unix)
            && let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty())
        {
            File::open(parent)?.sync_all()?;
        }
        self.file = OpenOptions::new().append(true).open(&self.path)?;
        self.schema = SCHEMA;
        self.append_ready = true;
        Ok(())
    }

    fn truncate(&self, length: u64) -> io::Result<()> {
        OpenOptions::new().write(true).open(&self.path)?.set_len(length)
    }
    fn persist(&mut self) {
        if self.pending.is_empty() || self.failed {
            return;
        }

        let written = || -> io::Result<()> {
            self.prepare_append()?;
            let start = self.file.metadata()?.len();
            let result = (|| -> io::Result<()> {
                for line in &self.pending {
                    writeln!(self.file, "{line}")?;
                }
                self.file.flush()?;
                self.file.sync_all()
            })();
            if result.is_err() && self.truncate(start).is_err() {
                self.failed = true;
            }
            result
        }();

        if written.is_ok() {
            self.pending.clear();
            self.checkpoint = self.written;
        }
    }

    fn prune(&mut self, beneath: Position) -> io::Result<()> {
        if beneath <= self.origin || self.checkpoint < beneath {
            return Ok(());
        }

        self.persist();
        if self.checkpoint < beneath {
            return Ok(());
        }

        let lines = self.read()?;
        let mut boundary = self.origin;
        let mut retained = Vec::new();
        for line in lines {
            let end = Position(members(&line)?.last().expect("validated group").position);
            if end <= beneath {
                boundary = end;
            } else {
                retained.push(line);
            }
        }
        if boundary == self.origin {
            return Ok(());
        }
        let beneath = boundary;
        let mut temporary = self.path.as_os_str().to_owned();
        temporary.push(".prune~");
        let temporary = PathBuf::from(temporary);
        let header = wire::Header { schema: SCHEMA, generation: self.generation, origin: beneath.0 };
        let mut fresh = File::create(&temporary)?;
        header.write(&mut fresh)?;
        for line in retained {
            writeln!(fresh, "{line}")?;
        }
        fresh.flush()?;
        fresh.sync_all()?;
        std::fs::rename(&temporary, &self.path)?;

        self.file = OpenOptions::new().append(true).open(&self.path)?;
        self.origin = beneath;
        self.schema = SCHEMA;

        Ok(())
    }

    fn reset(&mut self, generation: u64, at: Position) -> io::Result<()> {
        let fresh = Self::create(&self.path, generation, at)?;

        self.file = fresh.file;
        self.generation = generation;
        self.schema = SCHEMA;
        self.origin = at;
        self.pending.clear();
        self.written = at;
        self.checkpoint = at;

        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn is_group(line: &str) -> bool {
    #[derive(Deserialize)]
    struct Discriminator {
        group: Option<u32>,
    }
    serde_json::from_str::<Discriminator>(line).is_ok_and(|value| value.group.is_some())
}

fn members(line: &str) -> io::Result<Vec<wire::Line<serde_json::Value>>> {
    if line.len() > MAX_FRAME_BYTES {
        return Err(io::Error::other("journal frame exceeds byte limit"));
    }
    if !is_group(line) {
        return serde_json::from_str(line).map(|entry| vec![entry]).map_err(io::Error::other);
    }
    let group: Group<'_> = serde_json::from_str(line).map_err(io::Error::other)?;
    let count = group.frontier.checked_sub(group.first).and_then(|n| n.checked_add(1));
    let Some(count) = count.filter(|count| *count > 0 && *count <= (harmos::state::Records::MAX_RECORDS + 1) as u64) else {
        return Err(io::Error::other("invalid logical commit bounds"));
    };
    if group.group != 1 || group.first == 0 {
        return Err(io::Error::other("invalid logical commit bounds"));
    }
    use serde::de::DeserializeSeed;
    let entries =
        Members(count as usize).deserialize(&mut serde_json::Deserializer::from_str(group.entries.get())).map_err(io::Error::other)?;
    let witness = &entries[0].metadata;
    for (index, member) in entries.iter().enumerate() {
        if member.position != group.first + index as u64
            || !matches!((index, member.kind), (0, wire::Kind::Transaction) | (1.., wire::Kind::Record))
        {
            return Err(io::Error::other("invalid logical commit member"));
        }
        if index > 0
            && (member.metadata.principal != witness.principal
                || member.metadata.correlation != witness.correlation
                || member.metadata.at_us != witness.at_us
                || member.metadata.causation != Some(group.first)
                || member.metadata.request.is_some()
                || member.metadata.link.is_some())
        {
            return Err(io::Error::other("inconsistent logical commit witness"));
        }
    }
    Ok(entries)
}

fn migrated_member(stored: wire::Line<serde_json::Value>) -> Result<Envelope<serde_json::Value>, Incompatible> {
    Ok(Envelope {
        position: Position(stored.position),
        kind: stored.kind.into_kind(),
        id: DefinitionId::new(stored.id),
        version: stored.version,
        metadata: stored.metadata.witnessed().map_err(Incompatible::unreadable)?,
        payload: stored.payload,
    })
}

fn decoded<A: Wire>(envelope: Envelope<serde_json::Value>) -> Result<Option<Envelope<Payload<A>>>, Incompatible> {
    if envelope.kind == Kind::Record {
        match SystemFact::restore(&envelope.id, envelope.version, &envelope.payload) {
            Ok(Some(fact)) => {
                let id = envelope.id.clone();
                let version = envelope.version;

                return Ok(Some(Envelope {
                    position: envelope.position,
                    kind: envelope.kind,
                    id: id.clone(),
                    version,
                    metadata: envelope.metadata,
                    payload: Payload { prepared: None, id, version, value: Recorded::System(fact), apply: None },
                }));
            }
            Ok(None) => {}
            Err(refusal) => {
                return Err(Incompatible::unreadable(format!(
                    "{} version {} at {}: {refusal}",
                    envelope.id, envelope.version, envelope.position
                )));
            }
        }
    }

    let prepared = harmos::state::Prepared { id: envelope.id.clone(), version: envelope.version, payload: envelope.payload.clone() };
    let read = A::decode(envelope.kind, &envelope.id, envelope.version, &envelope.payload);
    let mut payload = match (envelope.kind, read) {
        (_, Ok(Some(payload))) => payload,
        (Kind::Record, _) => return Ok(None),
        (Kind::Transaction, Ok(None)) => {
            return Err(Incompatible::unreadable(format!(
                "{} version {} at {}: this build declares no implementation for it",
                envelope.id, envelope.version, envelope.position
            )));
        }
        (Kind::Transaction, Err(refusal)) => {
            return Err(Incompatible::unreadable(format!(
                "{} version {} at {}: {refusal}",
                envelope.id, envelope.version, envelope.position
            )));
        }
    };

    payload.prepared = Some(prepared);
    Ok(Some(Envelope {
        position: envelope.position,
        kind: envelope.kind,
        id: envelope.id,
        version: envelope.version,
        metadata: envelope.metadata,
        payload,
    }))
}

/* === Tests === */
