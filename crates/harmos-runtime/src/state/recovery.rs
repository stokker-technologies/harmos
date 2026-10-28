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
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio::sync::watch;
// Local Imports
use crate::state::{Inverting, Undos, dispatch::Dispatch, replay};
use crate::storage::{ReadLimits, RecoveryPosition, StorageManager};
use crate::{
    Application, runtime::Incompatible, state::At, state::Entry, state::Envelope, state::Kind, state::Position, state::Prepared,
    state::Receipt, state::RecordCatalog, state::Recorded, state::RequestKey, state::StateManager, state::SystemFact, storage::Payload,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Group<P> {
    version: u32,
    pub(crate) entries: Vec<Envelope<P>>,
}

pub(crate) struct Recovered<A: Application> {
    pub(crate) state: At<A::State>,
    pub(crate) undos: Undos<A>,
    pub(crate) tail: Vec<Arc<Entry<Recorded<A>>>>,
    pub(crate) durable: watch::Receiver<Position>,
    pub(crate) receipts: HashMap<RequestKey, Receipt>,
}

/* === Enums === */

enum RecoveryJson<'a> {
    Value(serde_json::Value),
    Raw(&'a RawValue),
}

/* === Traits === */

/* === Implementations === */

impl RecoveryJson<'_> {
    fn transaction<T>(&self, decode: impl FnOnce(&[u8]) -> T) -> std::result::Result<T, serde_json::Error> {
        match self {
            Self::Raw(raw) => Ok(decode(raw.get().as_bytes())),
            Self::Value(value) => serde_json::to_vec(value).map(|bytes| decode(&bytes)),
        }
    }

    fn system(&self, id: &crate::state::DefinitionId, version: u32) -> std::result::Result<Option<SystemFact>, serde_json::Error> {
        match self {
            Self::Value(value) => SystemFact::restore(id, version, value),
            Self::Raw(raw) => SystemFact::restore(id, version, &mut serde_json::Deserializer::from_str(raw.get())),
        }
    }

    fn value(self) -> std::result::Result<serde_json::Value, serde_json::Error> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Raw(raw) => serde_json::from_str(raw.get()),
        }
    }
}

impl<P> Group<P> {
    pub(crate) fn new(entries: Vec<Envelope<P>>) -> Self {
        Self { version: 1, entries }
    }
}

impl<A: Application> StateManager<A> {
    pub(crate) async fn recover<C>(
        storage: &StorageManager<C>, mut state: At<A::State>, catalog: &Dispatch<A, Payload<A>>, inverses: Inverting<A>, tail: usize,
        limits: ReadLimits, snapshots: Option<&super::snapshot::Snapshots<A>>,
    ) -> std::result::Result<Recovered<A>, Incompatible> {
        if state.position != Position::ORIGIN {
            return Err(Incompatible::unreadable("managed recovery requires the initial origin"));
        }
        let reader = storage
            .recovery_reader(limits, snapshots.map(|snapshots| snapshots.policy.max_bytes))
            .await
            .map_err(Incompatible::unreadable)?;
        let through = reader.through().clone();
        let mut after = RecoveryPosition {
            history: through.history.clone(),
            sequence: 0,
        };
        let mut undos = Undos::default();
        let mut receipts = HashMap::new();
        let mut retained = VecDeque::<Arc<Entry<Recorded<A>>>>::new();
        if let Some(snapshots) = snapshots
            && let Some(snapshot) = reader.snapshot().await.map_err(Incompatible::unreadable)?
        {
            if snapshot.payload.len() > snapshots.policy.max_bytes.get() {
                return Err(Incompatible::unreadable("invalid managed snapshot boundary or size"));
            }
            let restored = (snapshots.decode)(&snapshot.payload, &snapshots.policy, catalog, tail)?;
            if restored.state.position.0 < snapshot.through.sequence
                || restored.state.position.0 > snapshot.through.sequence.saturating_mul((crate::state::Records::MAX_RECORDS + 1) as u64)
            {
                return Err(Incompatible::unreadable("invalid snapshot logical frontier"));
            }
            after = snapshot.through.clone();
            state = restored.state;
            undos = restored.undos;
            receipts = restored.receipts;
            retained = restored.tail;
            snapshots.resumed(state.position);
        }
        while after.sequence < through.sequence {
            let page = reader.page(&after).await.map_err(Incompatible::unreadable)?;
            for entry in &page.entries {
                let group: Group<&RawValue> = serde_json::from_slice(&entry.payload).map_err(Incompatible::unreadable)?;
                group.check(state.position)?;
                let frontier = group.entries.last().expect("validated group").position;
                let first = group.entries.first().expect("validated group");
                if let Some(key) = &first.metadata.request {
                    let receipt = Receipt {
                        position: first.position,
                        frontier,
                        correlation: first.metadata.correlation,
                    };
                    if receipts.insert(key.clone(), receipt).is_some() {
                        return Err(Incompatible::unreadable("invalid or duplicate managed request key"));
                    }
                }
                let mut decoded = Vec::new();
                for envelope in group.entries {
                    if let Some(envelope) = decode_payload::<A>(envelope, catalog)? {
                        decoded.push(envelope);
                    }
                }
                let recovered = replay(Some(catalog), decoded, &mut state, &mut undos, inverses, tail).map_err(Incompatible::unreadable)?;
                state.position = frontier;
                if !recovered.is_empty() {
                    while retained.len() + recovered.len() > tail && !retained.is_empty() {
                        let end = retained.front().map(|entry| entry.metadata.commit.map_or(entry.position, |(_, end)| end)).unwrap();
                        while retained.front().is_some_and(|entry| entry.position <= end) {
                            retained.pop_front();
                        }
                    }
                    retained.extend(recovered);
                }
                after = entry.position.clone();
            }
        }
        let durable = reader.finish(state.position);
        Ok(Recovered {
            state,
            undos,
            tail: retained.into(),
            durable,
            receipts,
        })
    }
}
impl<P> Group<P> {
    fn check(&self, before: Position) -> std::result::Result<(), Incompatible> {
        let invalid = || Incompatible::unreadable("invalid managed recovery group");
        let first = self.entries.first().ok_or_else(invalid)?;
        let last = self.entries.last().ok_or_else(invalid)?;
        if self.entries.len() > crate::state::Records::MAX_RECORDS + 1 || before.0.checked_add(1) != Some(first.position.0) {
            return Err(invalid());
        }
        let commit = if first.kind == Kind::Transaction { Some((first.position, last.position)) } else { None };
        if commit.is_none() && self.entries.len() != 1 {
            return Err(invalid());
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if first.position.0.checked_add(index as u64) != Some(entry.position.0)
                || entry.metadata.commit != commit
                || (index != 0 && entry.kind != Kind::Record)
                || (entry.kind != Kind::Transaction && entry.metadata.request.is_some())
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn decode<A: Application>(
    envelope: Envelope<serde_json::Value>, catalog: &Dispatch<A, Payload<A>>,
) -> std::result::Result<Option<Envelope<Payload<A>>>, Incompatible> {
    let Envelope {
        position,
        kind,
        id,
        version,
        metadata,
        payload,
    } = envelope;
    decode_json(
        Envelope {
            position,
            kind,
            id,
            version,
            metadata,
            payload: RecoveryJson::Value(payload),
        },
        catalog,
    )
}

/* === Private Functions === */

fn decode_payload<A: Application>(
    envelope: Envelope<&RawValue>, catalog: &Dispatch<A, Payload<A>>,
) -> std::result::Result<Option<Envelope<Payload<A>>>, Incompatible> {
    let Envelope {
        position,
        kind,
        id,
        version,
        metadata,
        payload,
    } = envelope;
    decode_json(
        Envelope {
            position,
            kind,
            id,
            version,
            metadata,
            payload: RecoveryJson::Raw(payload),
        },
        catalog,
    )
}

fn decode_json<A: Application>(
    envelope: Envelope<RecoveryJson<'_>>, catalog: &Dispatch<A, Payload<A>>,
) -> std::result::Result<Option<Envelope<Payload<A>>>, Incompatible> {
    let Envelope {
        position,
        kind,
        id,
        version,
        metadata,
        payload,
    } = envelope;
    let (value, payload) = match kind {
        Kind::Transaction => {
            let transaction = payload
                .transaction(|bytes| catalog.decode_transaction(&id, version, bytes))
                .map_err(Incompatible::unreadable)?
                .map_err(Incompatible::unreadable)?;
            (Recorded::Transaction(crate::operations::Response::new(transaction)), payload.value().map_err(Incompatible::unreadable)?)
        }
        Kind::Record => {
            if let Some(system) = payload.system(&id, version).map_err(Incompatible::unreadable)? {
                (Recorded::System(system), payload.value().map_err(Incompatible::unreadable)?)
            } else if catalog.has_record(&id, version).map_err(Incompatible::unreadable)? {
                let payload = payload.value().map_err(Incompatible::unreadable)?;
                let record = A::Record::restore(&id, version, &payload)
                    .map_err(Incompatible::unreadable)?
                    .ok_or_else(|| Incompatible::unreadable("declared record could not be restored"))?;
                (Recorded::Record(record), payload)
            } else {
                return Ok(None);
            }
        }
    };
    let prepared = Prepared { id: id.clone(), version, payload };
    Ok(Some(Envelope {
        position,
        kind,
        id: id.clone(),
        version,
        metadata,
        payload: Payload {
            prepared: Some(prepared),
            id,
            version,
            value,
        },
    }))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    struct Model;

    #[crate::lanes]
    enum Lanes {
        #[default]
        Default,
    }
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
        type Streams = ();
    }

    #[test]
    fn raw_transaction_borrows() {
        let bytes = br#"{ "z": [3, 2, 1], "a": null }"#;
        let raw: &RawValue = serde_json::from_slice(bytes).unwrap();
        let result = RecoveryJson::Raw(raw)
            .transaction(|received| {
                assert_eq!(received.as_ptr(), raw.get().as_ptr());
                assert_eq!(received, bytes);
                received.len()
            })
            .unwrap();
        assert_eq!(result, bytes.len());
    }

    #[test]
    fn unknown_payload_skipped() {
        let raw: &RawValue = serde_json::from_str(r#"{"unknown":[[],{"nested":null}]}"#).unwrap();
        let envelope = Envelope {
            position: Position(1),
            kind: Kind::Record,
            id: crate::state::DefinitionId::new("unknown.record"),
            version: 1,
            metadata: crate::state::Metadata {
                commit: None,
                principal: None,
                correlation: crate::state::CorrelationId(Position(1)),
                causation: None,
                request: None,
                link: None,
                at: std::time::UNIX_EPOCH,
            },
            payload: raw,
        };
        let bytes = serde_json::to_vec(&Group::new(vec![envelope])).unwrap();
        let group: Group<&RawValue> = serde_json::from_slice(&bytes).unwrap();
        group.check(Position::ORIGIN).unwrap();
        let catalog = Dispatch::<Model, Payload<Model>>::default();
        assert!(decode_payload::<Model>(group.entries.into_iter().next().unwrap(), &catalog).unwrap().is_none());
    }
}
