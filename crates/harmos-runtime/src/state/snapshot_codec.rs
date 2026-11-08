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
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{self, Write};
use std::sync::Arc;

// Third Party Imports
use serde::{
    Deserialize, Serialize, Serializer,
    de::DeserializeOwned,
    ser::{SerializeSeq, SerializeStruct},
};

// Local Imports
use super::snapshot::{Restored, SnapshotPolicy};
use crate::state::{Metadata, Tail, UndoEntry, Undos, dispatch::Dispatch};
use crate::storage::SchemaKey;
use crate::storage::state::{SnapshotPayload, SnapshotWriter};
use crate::{
    Application, DefinitionId, Error, Result, runtime::Incompatible, state::At, state::CatalogCodec, state::Entry, state::Envelope,
    state::Kind, state::Position, state::Prepared, state::PrincipalId, state::Receipt, state::Recorded, state::RequestKey,
    storage::Payload,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<S> {
    definitions: Vec<(Kind, DefinitionId, u32)>,
    schema: SchemaKey,
    position: Position,
    state: S,
    undos: Vec<Undo>,
    receipts: Vec<(RequestKey, Receipt)>,
    tail: Vec<Envelope<serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Undo {
    position: Position,
    principal: PrincipalId,
    forward: Prepared,
    inverse: Option<Prepared>,
    undone: Option<Position>,
}

#[derive(Serialize)]
#[serde(bound(serialize = ""))]
struct UndoRef<'a, T: CatalogCodec> {
    position: Position,
    principal: &'a PrincipalId,
    forward: Leaf<'a, T>,
    inverse: Option<Leaf<'a, T>>,
    undone: Option<Position>,
}

#[derive(Serialize)]
struct EntryRef<'a> {
    position: Position,
    kind: Kind,
    id: &'a DefinitionId,
    version: u32,
    metadata: &'a Metadata,
    payload: &'a serde_json::Value,
}

struct Leaf<'a, T: CatalogCodec>(&'a T);

struct Body<'a, T: CatalogCodec>(&'a T);

struct Index<'a, A: Application>(&'a Undos<A>, usize, &'a Cell<Option<usize>>);

struct Window<'a, A: Application>(&'a Tail<A>, usize, &'a Cell<Option<usize>>);

struct Receipts<'a>(&'a HashMap<RequestKey, Receipt>, usize, &'a Cell<Option<usize>>);

struct Counter(usize);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T: CatalogCodec> Serialize for Body<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.encode(serializer)
    }
}

impl<T: CatalogCodec> Serialize for Leaf<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let (id, version) = self.0.identify();
        let mut value = serializer.serialize_struct("Prepared", 3)?;
        value.serialize_field("id", &id)?;
        value.serialize_field("version", &version)?;
        value.serialize_field("payload", &Body(self.0))?;
        value.end()
    }
}

impl<A: Application> Serialize for Index<'_, A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.entries.len()))?;
        for (position, entry) in &self.0.entries {
            let value = UndoRef {
                position: *position,
                principal: &entry.principal,
                forward: Leaf(entry.forward.as_ref()),
                inverse: entry.inverse.as_deref().map(Leaf),
                undone: entry.undone,
            };
            entry_size(&value, self.1, self.2)?;
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

impl<A: Application> Serialize for Window<'_, A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.entries.len()))?;
        for entry in &self.0.entries {
            let prepared = entry.prepared.as_ref().ok_or_else(|| serde::ser::Error::custom("unprepared snapshot entry"))?;
            let value = EntryRef {
                position: entry.position,
                kind: entry.value.kind(),
                id: &prepared.id,
                version: prepared.version,
                metadata: &entry.metadata,
                payload: &prepared.payload,
            };
            entry_size(&value, self.1, self.2)?;
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

impl Serialize for Receipts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for receipt in self.0 {
            entry_size(&receipt, self.1, self.2)?;
            sequence.serialize_element(&receipt)?;
        }
        sequence.end()
    }
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_add(bytes.len()).ok_or_else(|| io::Error::other("snapshot size overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn encode<A: Application>(
    policy: &SnapshotPolicy, catalog: &Dispatch<A, Payload<A>>, state: &At<A::State>, undos: &Undos<A>, tail: &Tail<A>,
    receipts: &HashMap<RequestKey, Receipt>,
) -> Result<SnapshotPayload>
where
    A::State: Serialize,
{
    let mut out = SnapshotWriter::new(policy.max_bytes).map_err(|source| Error::TaskFailed {
        domain: "state snapshot spool",
        source: Arc::new(source),
    })?;
    let oversized = Cell::new(None);
    let mut serializer = serde_json::Serializer::new(&mut out);
    let result = (|| {
        let mut value = serializer.serialize_struct("Snapshot", 7)?;
        value.serialize_field("definitions", &catalog.definitions())?;
        value.serialize_field("schema", &policy.schema)?;
        value.serialize_field("position", &state.position)?;
        value.serialize_field("undos", &Index(undos, policy.max_bytes.get(), &oversized))?;
        value.serialize_field("receipts", &Receipts(receipts, policy.max_bytes.get(), &oversized))?;
        value.serialize_field("tail", &Window(tail, policy.max_bytes.get(), &oversized))?;
        value.serialize_field("state", &state.value)?;
        SerializeStruct::end(value)
    })();
    if let Some(bytes) = oversized.get() {
        return Err(Error::SnapshotEntry { bytes, limit: policy.max_bytes.get() });
    }
    result.map_err(encoding_failure)?;
    out.finish(state.position).map_err(|source| Error::TaskFailed {
        domain: "state snapshot spool",
        source: Arc::new(source),
    })
}

pub(super) fn decode<A: Application>(
    bytes: &SnapshotPayload, policy: &SnapshotPolicy, catalog: &Dispatch<A, Payload<A>>, tail_limit: usize,
) -> std::result::Result<Restored<A>, Incompatible>
where
    A::State: DeserializeOwned,
{
    bytes.verify(policy.max_bytes.get()).map_err(Incompatible::unreadable)?;
    let reader = std::io::BufReader::new(bytes.reader().map_err(Incompatible::unreadable)?);
    let snapshot: Snapshot<A::State> = serde_json::from_reader(reader).map_err(Incompatible::unreadable)?;
    if snapshot.schema != policy.schema || bytes.manifest().position.is_some_and(|position| snapshot.position != position) {
        return Err(invalid());
    }
    for (kind, id, version) in &snapshot.definitions {
        catalog.validate(*kind, id, *version).map_err(Incompatible::of)?;
    }
    let mut entries = BTreeMap::new();
    let mut previous = Position::ORIGIN;
    for entry in snapshot.undos {
        if entry.position <= previous
            || entry.position > snapshot.position
            || entry.principal.as_str().len() > 1024
            || entry.undone.is_some_and(|at| at <= entry.position || at > snapshot.position)
        {
            return Err(invalid());
        }
        previous = entry.position;
        let forward = transaction(&entry.forward, catalog)?;
        let inverse = entry.inverse.as_ref().map(|value| transaction(value, catalog)).transpose()?;
        entries.insert(
            entry.position,
            UndoEntry {
                principal: entry.principal,
                forward,
                inverse,
                undone: entry.undone,
            },
        );
    }
    let mut receipts = HashMap::new();
    for (key, receipt) in snapshot.receipts {
        if key.as_str().len() > 1024
            || receipt.position == Position::ORIGIN
            || receipt.position > receipt.frontier
            || receipt.frontier > snapshot.position
            || receipts.insert(key, receipt).is_some()
        {
            return Err(invalid());
        }
    }
    let mut tail = VecDeque::new();
    previous = Position::ORIGIN;
    for envelope in snapshot.tail {
        if envelope.position <= previous
            || envelope.position > snapshot.position
            || (envelope.kind == Kind::Transaction && envelope.metadata.commit.is_none())
            || (envelope.kind == Kind::Record && envelope.metadata.request.is_some())
            || envelope.metadata.commit.is_some_and(|(first, end)| {
                first > envelope.position
                    || end < envelope.position
                    || end > snapshot.position
                    || (envelope.kind == Kind::Transaction && first != envelope.position)
            })
        {
            return Err(invalid());
        }
        previous = envelope.position;
        if let Some(entry) = super::recovery::decode::<A>(envelope, catalog)? {
            tail.push_back(Arc::new(Entry {
                position: entry.position,
                metadata: entry.metadata,
                prepared: entry.payload.prepared,
                value: entry.payload.value,
            }));
        }
    }
    trim(&mut tail, tail_limit);
    Ok(Restored {
        state: At {
            value: snapshot.state,
            position: snapshot.position,
        },
        undos: Undos { entries },
        receipts,
        tail,
    })
}

/* === Private Functions === */

fn transaction<A: Application>(
    value: &Prepared, catalog: &Dispatch<A, Payload<A>>,
) -> std::result::Result<crate::operations::Response<A::Transaction>, Incompatible> {
    let bytes = serde_json::to_vec(&value.payload).map_err(Incompatible::unreadable)?;
    catalog.decode_transaction(&value.id, value.version, &bytes).map(crate::operations::Response::new).map_err(Incompatible::of)
}

fn invalid() -> Incompatible {
    Incompatible::unreadable("invalid or incompatible managed state snapshot")
}

fn entry_size<E: serde::ser::Error>(value: &impl Serialize, limit: usize, oversized: &Cell<Option<usize>>) -> std::result::Result<(), E> {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(E::custom)?;
    if counter.0 > limit {
        oversized.set(Some(counter.0));
        return Err(E::custom("snapshot entry exceeds segment bound"));
    }
    Ok(())
}

fn encoding_failure(error: serde_json::Error) -> Error {
    Error::TaskFailed {
        domain: "state snapshot encoding",
        source: Arc::new(error),
    }
}

fn trim<A: Application>(tail: &mut VecDeque<Arc<Entry<Recorded<A>>>>, limit: usize) {
    if limit == 0 {
        tail.clear();
        return;
    }
    while tail.len() > limit {
        let first = tail.front().unwrap();
        let end = first.metadata.commit.map_or(first.position, |(_, end)| end);
        if tail.back().unwrap().position <= end {
            break;
        }
        while tail.front().is_some_and(|entry| entry.position <= end) {
            tail.pop_front();
        }
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    struct Refusing;

    impl Serialize for Refusing {
        fn serialize<S: Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("a map key must be a string"))
        }
    }

    #[test]
    fn serialization_cause_preserved() {
        let mut out = Counter(0);
        let error = encoding_failure(serde_json::to_writer(&mut out, &Refusing).unwrap_err());
        assert!(matches!(&error, Error::TaskFailed { domain: "state snapshot encoding", .. }), "{error:?}");
        assert!(std::error::Error::source(&error).unwrap().to_string().contains("a map key must be a string"));
    }
}
