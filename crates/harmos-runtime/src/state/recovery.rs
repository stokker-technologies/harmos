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

/* === Traits === */

/* === Implementations === */

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
        let mut after = RecoveryPosition { history: through.history.clone(), sequence: 0 };
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
            after = snapshot.through;
            state = restored.state;
            undos = restored.undos;
            receipts = restored.receipts;
            retained = restored.tail;
            snapshots.resumed(state.position);
        }
        while after.sequence < through.sequence {
            let page = reader.page(&after).await.map_err(Incompatible::unreadable)?;
            for entry in page.entries {
                let group: Group<serde_json::Value> = serde_json::from_slice(&entry.payload).map_err(Incompatible::unreadable)?;
                group.check(state.position)?;
                let frontier = group.entries.last().expect("validated group").position;
                let first = group.entries.first().expect("validated group");
                if let Some(key) = &first.metadata.request {
                    let receipt = Receipt { position: first.position, frontier, correlation: first.metadata.correlation };
                    if receipts.insert(key.clone(), receipt).is_some() {
                        return Err(Incompatible::unreadable("invalid or duplicate managed request key"));
                    }
                }
                let mut decoded = Vec::new();
                for envelope in group.entries {
                    if let Some(envelope) = decode::<A>(envelope, catalog)? {
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
                after = entry.position;
            }
        }
        let durable = reader.finish(state.position);
        Ok(Recovered { state, undos, tail: retained.into(), durable, receipts })
    }
}
impl Group<serde_json::Value> {
    fn check(&self, before: Position) -> std::result::Result<(), Incompatible> {
        let invalid = || Incompatible::unreadable("invalid managed recovery group");
        let first = self.entries.first().ok_or_else(invalid)?;
        let last = self.entries.last().ok_or_else(invalid)?;
        if self.version != 1
            || self.entries.len() > crate::state::Records::MAX_RECORDS + 1
            || before.0.checked_add(1) != Some(first.position.0)
        {
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
    let Envelope { position, kind, id, version, metadata, payload } = envelope;
    let value = match kind {
        Kind::Transaction => {
            let bytes = serde_json::to_vec(&payload).map_err(Incompatible::unreadable)?;
            let transaction = catalog.decode_transaction(&id, version, &bytes).map_err(Incompatible::unreadable)?;
            Recorded::Transaction(Arc::new(transaction))
        }
        Kind::Record => {
            if let Some(system) = SystemFact::restore(&id, version, &payload).map_err(Incompatible::unreadable)? {
                Recorded::System(system)
            } else if catalog.has_record(&id, version).map_err(Incompatible::unreadable)? {
                let record = A::Record::restore(&id, version, &payload)
                    .map_err(Incompatible::unreadable)?
                    .ok_or_else(|| Incompatible::unreadable("declared record could not be restored"))?;
                Recorded::Record(record)
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
        payload: Payload { prepared: Some(prepared), id, version, value, apply: None },
    }))
}

/* === Private Functions === */

/* === Tests === */
