/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Durable Journal Copy And Snapshot Fold For Stories That Restart A Runtime.
// Frames live in a shared store, so a second boot reopens what the first one
// synced, and whatever a crash left in a copy's batch never reaches it.

#![allow(dead_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::io;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};
// Third Party Imports
use serde::{Deserializer, Serializer};
// Local Imports
use harmos_runtime::{
    Application, DefinitionId, Error, runtime::Incompatible, state::Entry, state::Envelope, state::Kind, state::Position, state::Prepared,
    state::Recorded, state::SystemFact, storage::JournalConsumer, storage::Payload, storage::Recovery, storage::Source,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Member = Envelope<serde_json::Value>;

type Sink<A> = Box<dyn FnMut(&<A as Application>::State, Position) -> io::Result<()> + Send>;

/* === Structs === */

/// The synced frames of one copy: each frame is one JSON array holding a
/// standalone entry or every member of one logical commit.
#[derive(Clone, Default)]
pub struct Store(Arc<Mutex<Frames>>);

#[derive(Default)]
struct Frames {
    origin: Position,
    lines: Vec<String>,
}

pub struct Journal<A: Wire> {
    store: Store,
    batch: usize,
    after: Option<Position>,
    forward: BTreeMap<Position, String>,
    halted: bool,
    pending: Vec<String>,
    written: Position,
    checkpoint: Position,
    application: PhantomData<fn() -> A>,
}

/// Folds a replica of state and saves it whenever a whole commit is reached.
pub struct Snapshot<A: Application> {
    replica: A::State,
    taken: Position,
    durable: Position,
    complete: Position,
    halted: bool,
    write: Sink<A>,
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

impl Store {
    pub fn lines(&self) -> Vec<String> {
        self.frames().lines.clone()
    }

    pub fn origin(&self) -> Position {
        self.frames().origin
    }

    pub fn push(&self, line: impl Into<String>) {
        self.frames().lines.push(line.into());
    }

    pub fn replace(&self, lines: Vec<String>) {
        self.frames().lines = lines;
    }

    /// Every stored member, groups flattened, as the JSON it was synced as.
    pub fn members(&self) -> Vec<serde_json::Value> {
        self.lines()
            .iter()
            .flat_map(|line| {
                let frame: Vec<serde_json::Value> = serde_json::from_str(line).expect("a stored frame is a JSON array");
                frame
            })
            .collect()
    }

    fn frames(&self) -> MutexGuard<'_, Frames> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<A: Wire> Journal<A> {
    pub fn open(store: &Store) -> Self {
        let frames = store.frames();
        let written = frames
            .lines
            .last()
            .and_then(|line| members(line).ok())
            .and_then(|group| group.last().map(|member| member.position))
            .unwrap_or(frames.origin);
        drop(frames);

        Self {
            store: store.clone(),
            batch: 1,
            after: None,
            forward: BTreeMap::new(),
            halted: false,
            pending: Vec::new(),
            written,
            checkpoint: written,
            application: PhantomData,
        }
    }

    pub fn after(mut self, at: Position) -> Self {
        self.after = Some(at);
        self
    }

    pub fn syncing_every(mut self, entries: usize) -> Self {
        self.batch = entries.max(1);
        self
    }

    fn encode(&self, entry: &Entry<Recorded<A>>) -> Result<Member, serde_json::Error> {
        let (id, version, payload) = if let Some(prepared) = &entry.prepared {
            (prepared.id.clone(), prepared.version, prepared.payload.clone())
        } else if let Recorded::System(fact) = &entry.value {
            let (id, version) = fact.identify();
            let payload = serde::Serialize::serialize(&fact.payload, serde_json::value::Serializer)?;

            (id, version, payload)
        } else {
            let (id, version) = A::identify(&entry.value);
            let payload = A::encode(&entry.value, serde_json::value::Serializer)?;

            (id, version, payload)
        };

        Ok(Envelope { position: entry.position, kind: entry.value.kind(), id, version, metadata: entry.metadata.clone(), payload })
    }

    fn persist(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut frames = self.store.frames();
        frames.lines.append(&mut self.pending);
        self.checkpoint = self.written;
    }

    fn halt(&mut self) -> Position {
        self.halted = true;
        self.checkpoint
    }
}

impl<A: Wire> JournalConsumer<A> for Journal<A> {
    fn resume(&self) -> Position {
        self.written
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, _applied: Position) -> Position {
        // A halted copy refuses to advance: the stalled checkpoint is the
        // loud form of the gap it refused to write across.
        if self.halted {
            return self.checkpoint;
        }

        // Forwarded Frames Are Written Exactly As Stored
        let mut frames = BTreeMap::new();
        for (_, line) in std::mem::take(&mut self.forward) {
            let Ok(group) = members(&line) else {
                return self.halt();
            };
            let first = group[0].position;
            let end = group.last().expect("a validated frame").position;
            frames.insert(first, (end, line));
        }

        // Every Other Entry Is Encoded, One Whole Commit Per Frame
        let mut index = 0;
        while index < entries.len() {
            let entry = &entries[index];
            if frames.range(..=entry.position).next_back().is_some_and(|(_, (end, _))| *end >= entry.position) {
                index += 1;
                continue;
            }
            let (first, end) = entry.metadata.commit.unwrap_or((entry.position, entry.position));
            if first != entry.position {
                return self.halt();
            }
            let count = usize::try_from(end.0 - first.0 + 1).unwrap_or(usize::MAX);
            let Some(group) = entries.get(index..).and_then(|remaining| remaining.get(..count)) else {
                return self.halt();
            };
            let encoded = group.iter().map(|member| self.encode(member)).collect::<Result<Vec<_>, _>>();
            let Ok(line) = encoded.and_then(|group| serde_json::to_string(&group)) else {
                return self.halt();
            };
            frames.insert(first, (end, line));
            index += count;
        }

        // Frames Append Only Where The Copy Ends
        for (first, (end, line)) in frames {
            if Some(first.0) != self.written.0.checked_add(1) {
                self.halted = true;
                break;
            }
            self.pending.push(line);
            self.written = end;
        }
        if self.pending.len() >= self.batch {
            self.persist();
        }

        self.checkpoint
    }

    async fn flush(&mut self) {
        // The held batch is this copy's declared risk window; stop closes it,
        // so a clean stop loses nothing however lazily the copy was syncing.
        self.persist();
    }

    fn redelivered(&mut self, stored: &[(Position, Vec<u8>)]) {
        if self.halted {
            return;
        }
        for (position, bytes) in stored {
            let Ok(line) = std::str::from_utf8(bytes) else {
                continue;
            };
            if members(line).is_ok() {
                self.forward.insert(*position, line.to_owned());
            }
        }
    }
}

impl<A: Wire> Source<A> for Journal<A> {
    async fn load(&mut self) -> Result<Recovery<A>, Incompatible> {
        let (stored, horizon) = {
            let frames = self.store.frames();
            (frames.lines.clone(), frames.origin)
        };
        let origin = self.after.unwrap_or(horizon).max(horizon);
        let mut recovery = Recovery::default();
        let mut expected = horizon.0 + 1;

        for line in stored {
            let group = members(&line).map_err(Incompatible::unreadable)?;
            let first = group[0].position;
            let end = group.last().expect("a validated frame").position;
            if first.0 != expected {
                return Err(Incompatible::unreadable("journal position gap"));
            }
            expected = end.0 + 1;
            if first <= origin && origin < end {
                return Err(Incompatible::unreadable("recovery origin splits a logical commit"));
            }
            if end <= origin {
                continue;
            }
            self.written = end;
            self.checkpoint = end;
            recovery.encoded.push((end, line.into_bytes()));
            if group[0].metadata.commit.is_some() {
                recovery.commits.push((first, end));
            }
            for member in group {
                if let Some(entry) = decoded::<A>(member)? {
                    recovery.entries.push(entry);
                }
            }
        }
        self.written = self.written.max(origin);
        self.checkpoint = self.checkpoint.max(origin);
        recovery.frontier = self.written;

        Ok(recovery)
    }

    async fn read(&mut self, after: Position, through: Position) -> Result<Recovery<A>, Error> {
        let (stored, horizon) = {
            let frames = self.store.frames();
            (frames.lines.clone(), frames.origin)
        };
        if after < horizon {
            return Err(Error::BeneathHorizon);
        }
        if through > self.checkpoint {
            return Err(Error::BeyondTail);
        }
        let mut recovery = Recovery::default();

        for line in stored {
            let group = members(&line).map_err(|_| Error::Unavailable)?;
            let first = group[0].position;
            let end = group.last().expect("a validated frame").position;
            if end <= after || first > through {
                continue;
            }
            if group[0].metadata.commit.is_some() {
                recovery.commits.push((first, end));
            }
            if first > after && end <= through {
                recovery.encoded.push((end, line.into_bytes()));
            }
            for member in group {
                if member.position <= after || member.position > through {
                    continue;
                }
                if let Some(entry) = decoded::<A>(member).map_err(|_| Error::Unavailable)? {
                    recovery.entries.push(entry);
                }
            }
        }
        recovery.frontier = through;

        Ok(recovery)
    }

    async fn prune(&mut self, beneath: Position) {
        self.persist();
        if self.checkpoint < beneath {
            return;
        }
        let mut frames = self.store.frames();
        if beneath <= frames.origin {
            return;
        }
        let mut boundary = frames.origin;
        let mut retained = Vec::new();
        for line in std::mem::take(&mut frames.lines) {
            let end = members(&line).ok().and_then(|group| group.last().map(|member| member.position)).unwrap_or(Position::ORIGIN);
            if end <= beneath {
                boundary = end;
            } else {
                retained.push(line);
            }
        }
        frames.lines = retained;
        frames.origin = boundary;
    }
}

impl<A: Application> Snapshot<A> {
    pub fn folding(origin: A::State, at: Position, write: impl FnMut(&A::State, Position) -> io::Result<()> + Send + 'static) -> Self {
        Self { replica: origin, taken: at, durable: at, complete: at, halted: false, write: Box::new(write) }
    }
}

impl<A: Application> JournalConsumer<A> for Snapshot<A> {
    fn resume(&self) -> Position {
        self.taken
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, applied: Position) -> Position {
        if self.halted {
            return self.durable;
        }
        for entry in &entries {
            if let Recorded::Transaction(change) = &entry.value {
                // Without original bytes a carried historical value cannot
                // prove which apply produced the accepted state.
                let replayed = entry.prepared.as_ref().is_some_and(|prepared| {
                    harmos_runtime::state::TransactionCatalog::replay(&**change, prepared, &mut self.replica).is_ok()
                });
                if !replayed {
                    self.halted = true;
                    return self.durable;
                }
            }
            self.taken = entry.position;
            if entry.metadata.commit.is_none_or(|(_, end)| end == entry.position) {
                self.complete = entry.position;
            }
        }
        // The source may omit unknown passive records. Its complete applied
        // frontier still proves the containing group is fully available.
        if let Some((_, end)) = entries.last().and_then(|entry| entry.metadata.commit)
            && end <= applied
        {
            self.taken = end;
            self.complete = end;
        }
        if self.complete == self.taken && self.durable < self.taken && (self.write)(&self.replica, self.taken).is_ok() {
            self.durable = self.taken;
        }

        self.durable
    }
}

/* === Public Functions === */

/// One standalone frame, as some build stored it.
pub fn frame(position: u64, kind: Kind, id: &str, version: u32, payload: serde_json::Value) -> String {
    let member = serde_json::json!({
        "position": position,
        "kind": kind,
        "id": id,
        "version": version,
        "metadata": {
            "principal": null,
            "correlation": position,
            "causation": null,
            "request": null,
            "link": null,
            "at": { "secs_since_epoch": 0, "nanos_since_epoch": 0 },
        },
        "payload": payload,
    });

    serde_json::to_string(&[member]).expect("a frame is JSON")
}

/* === Private Functions === */

fn members(line: &str) -> Result<Vec<Member>, String> {
    let group: Vec<Member> = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let Some(first) = group.first() else {
        return Err("an empty frame".to_owned());
    };
    let consecutive = group.iter().enumerate().all(|(offset, member)| member.position.0 == first.position.0 + offset as u64);
    if !consecutive {
        return Err("a frame's members are not consecutive".to_owned());
    }

    Ok(group)
}

fn decoded<A: Wire>(member: Member) -> Result<Option<Envelope<Payload<A>>>, Incompatible> {
    let unreadable = |reason: &dyn std::fmt::Display| {
        Incompatible::unreadable(format!("{} version {} at {}: {reason}", member.id, member.version, member.position))
    };

    // System Facts Decode Through Harmos Itself
    if member.kind == Kind::Record {
        match SystemFact::restore(&member.id, member.version, &member.payload) {
            Ok(Some(fact)) => {
                let payload =
                    Payload { prepared: None, id: member.id.clone(), version: member.version, value: Recorded::System(fact), apply: None };
                return Ok(Some(envelope(member, payload)));
            }
            Ok(None) => {}
            Err(refusal) => return Err(unreadable(&refusal)),
        }
    }

    // Unknown Records Are Skipped; Unknown Changes End The Boot
    let read = A::decode(member.kind, &member.id, member.version, &member.payload);
    let mut payload = match (member.kind, read) {
        (_, Ok(Some(payload))) => payload,
        (Kind::Record, _) => return Ok(None),
        (Kind::Transaction, Ok(None)) => {
            return Err(unreadable(&"this build declares no implementation for it"));
        }
        (Kind::Transaction, Err(refusal)) => return Err(unreadable(&refusal)),
    };
    payload.prepared = Some(Prepared { id: member.id.clone(), version: member.version, payload: member.payload.clone() });

    Ok(Some(envelope(member, payload)))
}

fn envelope<P>(member: Member, payload: P) -> Envelope<P> {
    Envelope { position: member.position, kind: member.kind, id: member.id, version: member.version, metadata: member.metadata, payload }
}

/* === Tests === */
