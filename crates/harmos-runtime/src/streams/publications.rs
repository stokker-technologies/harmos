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
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::definition::{Contract, Windows};
use super::{Demand, Holding, Item, Query, StreamManager, Wired, window};
use crate::{Application, state::Position, streams::Definition};
use harmos_operations::StreamDeclaration;
use harmos_operations::inventory::publications;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationRow {
    pub payload: Vec<u8>,
    pub published_unix_micros: Option<i64>,

    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Clone)]
pub struct PublishedStream {
    pub(super) wired: Arc<Wired>,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) pool: Arc<super::publication::Pool>,
    pub(super) ingest: Arc<super::ingest::Ingest>,
}

pub struct StreamWriter {
    pub(super) stream: PublishedStream,
    pub(super) active: Arc<Mutex<bool>>,
    pub(super) applied: watch::Receiver<Position>,
    pub(super) sequence: AtomicU64,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum StreamRegistrationError {
    #[error("`{0}` is not a portable plugin owner identity")]
    Owner(String),

    #[error(transparent)]
    Declaration(#[from] publications::Error),

    #[error("stream `{0}` is already owned by a live publication lease")]
    Occupied(String),

    #[error("stream `{0}` belongs to another plugin; use a stream id owned by this plugin")]
    OwnerConflict(String),

    #[error("stream `{0}` has a revision hash collision; rename the stream id and report this collision")]
    Collision(String),

    #[error("stream `{0}` collides with a native catalog definition; choose a different stream id")]
    Native(String),

    #[error("stream inventory cannot change during a transport restart; reload the plugin to activate a changed inventory")]
    Inventory,

    #[error("the stream facility has stopped")]
    Closed,

    #[error(transparent)]
    Schema(#[from] crate::schema::Error),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum StreamPublicationError {
    #[error("the publication lease is no longer active")]
    Revoked,

    #[error("a plugin publication exceeds the 1 MiB row limit")]
    Oversized,

    #[error("the shared plugin publication byte or slot capacity is full")]
    Capacity,

    #[error("the publication sequence is invalid or exhausted")]
    Sequence,

    #[error("a batched row is stamped earlier than the row before it")]
    Stamp,

    #[error("the stream window is full of rows its ingest has not taken in")]
    Backlog,

    #[error("the plugin activation has not committed")]
    NotReady,

    #[error(transparent)]
    Payload(#[from] crate::schema::PayloadError),
}

/* === Traits === */

/* === Implementations === */

impl<A: Application> StreamManager<A> {
    pub fn validate_publications(
        &self, owner: &str, declarations: impl IntoIterator<Item = StreamDeclaration>,
    ) -> Result<(), StreamRegistrationError> {
        let definitions = publication_definitions(owner, declarations)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(StreamRegistrationError::Closed);
        }
        let _visibility = self.windows.visibility.read().expect("stream visibility poisoned");
        let windows = self.windows.windows.read().expect("stream registry poisoned");
        validate_existing(&windows, &definitions, false)
    }

    pub fn publication(&self, id: &str, version: u32) -> Option<PublishedStream> {
        let wired = self.windows.wired(id, version)?;
        wired.definition.owner()?;

        Some(PublishedStream {
            wired,
            closed: Arc::clone(&self.closed),
            pool: self.windows.publications.clone(),
            ingest: self.windows.ingest.clone(),
        })
    }

    pub fn current_publication(&self, id: &str) -> Option<PublishedStream> {
        let wired = self.windows.current(id)?;
        wired.definition.owner()?;
        Some(PublishedStream {
            wired,
            closed: Arc::clone(&self.closed),
            pool: self.windows.publications.clone(),
            ingest: self.windows.ingest.clone(),
        })
    }

    pub fn definitions(&self) -> Vec<Definition> {
        let mut definitions: Vec<_> = self.windows.declared().into_iter().map(|wired| wired.definition.clone()).collect();
        definitions.sort_by(|a, b| a.id().cmp(b.id()));
        definitions
    }
}

impl StreamWriter {
    pub fn stream(&self) -> &PublishedStream {
        &self.stream
    }

    pub fn publish(&self, payload: Vec<u8>, missed: u64) -> Result<(), StreamPublicationError> {
        self.publish_row(
            PublicationRow {
                payload,
                published_unix_micros: None,
                context: None,
            },
            missed,
        )
    }

    pub fn publish_row(&self, row: PublicationRow, missed: u64) -> Result<(), StreamPublicationError> {
        self.admit(row, None, missed, None).map(|_| ())
    }

    pub fn admit_row(
        &self, row: PublicationRow, wire_sequence: u64, missed: u64,
    ) -> Result<super::ManagedPublication, StreamPublicationError> {
        self.admit(row, Some(wire_sequence), missed, None)
    }

    pub(crate) fn admit_clocked_row(
        &self, row: PublicationRow, wire_sequence: u64, clock: &crate::clock::Clock,
    ) -> Result<super::ManagedPublication, StreamPublicationError> {
        self.admit(row, Some(wire_sequence), 0, Some(clock.activity("stream storage delivery")))
    }

    // An ordered batch takes contiguous sequences from `first_wire_sequence`;
    // `missed` applies before the first row only.
    #[cfg(feature = "plugin-guest")]
    pub fn stage_rows(&self, rows: Vec<PublicationRow>, first_wire_sequence: u64, missed: u64) -> super::StagedRows {
        self.stage_sequence(rows, Some(first_wire_sequence), missed, None)
    }

    #[cfg(feature = "plugin-guest")]
    pub fn stage_derived_row(&self, row: PublicationRow, missed: u64) -> Result<super::StagedPublication, StreamPublicationError> {
        self.stage_sequence(vec![row], None, missed, None).single()
    }

    // One visibility fence and one scope phase cover the whole batch, so no
    // reader or commit observes it half staged.
    #[cfg(feature = "plugin-guest")]
    fn stage_sequence(
        &self, rows: Vec<PublicationRow>, wire_sequence: Option<u64>, missed: u64, context: Option<&crate::operations::Context<'_>>,
    ) -> super::StagedRows {
        let Some(staged) = self.stream.wired.staged.as_ref() else {
            return super::StagedRows::refused(Vec::new(), StreamPublicationError::NotReady);
        };
        let _visibility = self.stream.wired.window.visibility.write().expect("stream visibility poisoned");
        let mut evicted = Vec::new();
        let prepared = staged.scope.prepare_publication_in(context, |ready| {
            let activation = staged.activation.upgrade().ok_or(StreamPublicationError::Revoked)?;
            let mut receipts = Vec::with_capacity(rows.len());
            let mut stamped = None;
            // Staged Rows Join The Window At Commit; They Must Evict No Unread Row.
            let mut room = if ready {
                usize::MAX
            } else {
                self.stream.wired.window.room(self.stream.ingest.taken()).saturating_sub(activation.pending(&self.stream.wired))
            };
            for (offset, row) in (0_u64..).zip(rows) {
                if room == 0 {
                    return Ok((receipts, ready, Some(StreamPublicationError::Backlog)));
                }
                room -= 1;
                let sequence = match wire_sequence.map(|first| first.checked_add(offset)) {
                    Some(None) => {
                        return Ok((receipts, ready, Some(StreamPublicationError::Sequence)));
                    }
                    sequence => sequence.flatten(),
                };
                // Stamps Within One Batch Never Go Back
                if let Some(stamp) = row.published_unix_micros {
                    if stamped.is_some_and(|previous| stamp < previous) {
                        return Ok((receipts, ready, Some(StreamPublicationError::Stamp)));
                    }
                    stamped = Some(stamp);
                }
                let missed = if offset == 0 { missed } else { 0 };
                let (publication, dropped) = match self.admit_locked(row, sequence, missed, ready) {
                    Ok(admitted) => admitted,
                    Err(error) => return Ok((receipts, ready, Some(error))),
                };
                evicted.extend(dropped);
                receipts.push(if ready {
                    let (_, committed) = watch::channel(true);
                    super::StagedPublication { publication, committed }
                } else {
                    activation.push(&self.stream.wired, *self.applied.borrow(), publication)
                });
            }
            Ok((receipts, ready, None))
        });
        drop(_visibility);
        drop(evicted);
        let (receipts, ready, refused) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return super::StagedRows::refused(Vec::new(), error),
        };
        if ready && !receipts.is_empty() {
            self.stream.wired.window.knock();
            self.stream.wired.window.wake_delivery();
        }
        match refused {
            Some(error) => super::StagedRows::refused(receipts, error),
            None => super::StagedRows::staged(receipts),
        }
    }

    fn admit(
        &self, row: PublicationRow, sequence: Option<u64>, missed: u64, activity: Option<crate::clock::Activity>,
    ) -> Result<super::ManagedPublication, StreamPublicationError> {
        let _visibility = self.stream.wired.window.visibility.write().expect("stream visibility poisoned");
        let result = {
            #[cfg(not(feature = "plugin-guest"))]
            {
                self.admit_locked(row, sequence, missed, true)
            }
            #[cfg(feature = "plugin-guest")]
            if let Some(staged) = &self.stream.wired.staged {
                staged.scope.prepare_publication(|ready| {
                    if !ready {
                        return Err(StreamPublicationError::NotReady);
                    }
                    self.admit_locked(row, sequence, missed, true)
                })
            } else {
                self.admit_locked(row, sequence, missed, true)
            }
        };
        let (publication, evicted) = result?;
        if let Some(activity) = activity {
            self.stream.ingest.track(self.stream.wired.window.latest_locked(), activity);
        }
        drop(_visibility);
        drop(evicted);
        self.stream.wired.window.knock();
        self.stream.wired.window.wake_delivery();
        Ok(publication)
    }

    // Visibility and original scope phase precede writer and row-state locks.
    fn admit_locked(
        &self, row: PublicationRow, sequence: Option<u64>, missed: u64, live: bool,
    ) -> Result<(super::ManagedPublication, Option<super::window::Row>), StreamPublicationError> {
        let active = self.active.lock().expect("stream lease poisoned");
        if !*active || self.stream.closed.load(Ordering::Acquire) {
            return Err(StreamPublicationError::Revoked);
        }
        let next = self
            .sequence
            .load(Ordering::Relaxed)
            .checked_add(missed)
            .and_then(|sequence| sequence.checked_add(1))
            .ok_or(StreamPublicationError::Sequence)?;
        if sequence.is_some_and(|sequence| sequence != next) {
            return Err(StreamPublicationError::Sequence);
        }
        self.stream.wired.validation.as_ref().expect("compiled plugin schema").validate(&row.payload)?;
        let generation = {
            #[cfg(not(feature = "plugin-guest"))]
            {
                None
            }
            #[cfg(feature = "plugin-guest")]
            {
                self.stream.wired.staged.as_ref().map(|staged| staged.scope.generation().get())
            }
        };
        if live && !self.stream.wired.window.can_advance(missed.checked_add(1).ok_or(StreamPublicationError::Sequence)?) {
            return Err(StreamPublicationError::Sequence);
        }
        let taken = self.stream.ingest.taken();
        if live && self.stream.wired.window.evicts_unread(taken) {
            return Err(StreamPublicationError::Backlog);
        }
        let publication = self.stream.pool.retain(
            &self.stream.wired.definition,
            generation,
            row,
            next,
            missed,
            live.then_some((self.stream.wired.window.as_ref(), taken)),
        )?;
        let evicted = if live {
            self.stream.wired.window.append_locked(*self.applied.borrow(), Arc::new(publication.clone()), missed)?
        } else {
            None
        };
        self.sequence.store(next, Ordering::Relaxed);
        Ok((publication, evicted))
    }

    pub fn revoke(&self) {
        #[cfg(feature = "plugin-guest")]
        if let Some(staged) = &self.stream.wired.staged {
            staged.scope.unbind_streams();
        }
        *self.active.lock().expect("stream lease poisoned") = false;
        self.stream.ingest.wake();
    }

    // Wait until every live feed has taken in every row published on this
    // stream so far. A caught-up stream answers without yielding.
    pub async fn ingested(&self) -> Result<(), StreamPublicationError> {
        let mut taken = self.stream.ingest.watch();
        loop {
            if !*self.active.lock().expect("stream lease poisoned") || self.stream.closed.load(Ordering::Acquire) {
                return Err(StreamPublicationError::Revoked);
            }
            let published = self.stream.wired.window.latest_locked();
            if taken.borrow_and_update().values().all(|cursor| *cursor >= published) {
                return Ok(());
            }
            taken.changed().await.map_err(|_| StreamPublicationError::Revoked)?;
        }
    }
}

impl Drop for StreamWriter {
    fn drop(&mut self) {
        self.revoke();
    }
}

impl PublishedStream {
    pub fn definition(&self) -> &Definition {
        &self.wired.definition
    }

    pub fn query(&self, range: impl RangeBounds<Position>) -> Query<PublicationRow> {
        Query {
            window: Arc::clone(&self.wired.window),
            start: range.start_bound().cloned(),
            end: range.end_bound().cloned(),
            as_of: None,
            retained_only: false,
            marker: PhantomData,
        }
    }

    pub fn watch(&self) -> impl futures_core::Stream<Item = Item<PublicationRow>> + Send + Unpin + use<> {
        let window = Arc::clone(&self.wired.window);
        let holding = Holding::engage(Arc::clone(&window));
        window::following(window, Arc::clone(&self.closed), holding)
    }

    pub fn demand(&self) -> Demand {
        Demand::of(&self.wired.window)
    }
}

/* === Public Functions === */

pub(super) fn publication_definitions(
    owner: &str, declarations: impl IntoIterator<Item = StreamDeclaration>,
) -> Result<Vec<Definition>, StreamRegistrationError> {
    if !owner.split('.').all(harmos_domain::identity::segment) {
        return Err(StreamRegistrationError::Owner(owner.into()));
    }
    let mut ids = BTreeSet::new();
    declarations
        .into_iter()
        .map(|declaration| {
            publications::validate(&declaration)?;
            if !ids.insert(declaration.id.clone()) {
                return Err(publications::Error::DuplicateStream { id: declaration.id }.into());
            }
            let definition = Definition::declared::<PublicationRow>(declaration.id, declaration.row, declaration.window)
                .map_err(crate::schema::Error::from)?;
            let definition = definition.presentation(declaration.metadata, declaration.row_metadata).publication(owner);
            Ok(definition)
        })
        .collect()
}

pub(super) fn validate_existing(
    windows: &Windows, definitions: &[Definition], require_unoccupied: bool,
) -> Result<(), StreamRegistrationError> {
    for definition in definitions {
        let Some(revisions) = windows.get(definition.id()) else {
            continue;
        };
        if revisions.definitions.values().any(|defined| defined.owner.is_none()) {
            return Err(StreamRegistrationError::Native(definition.id().into()));
        }
        if revisions.definitions.values().any(|defined| defined.owner.as_deref() != definition.owner()) {
            return Err(StreamRegistrationError::OwnerConflict(definition.id().into()));
        }
        let Some(existing) = revisions.definitions.get(&definition.version()) else {
            continue;
        };
        if existing != &Contract::of(definition) {
            return Err(StreamRegistrationError::Collision(definition.id().into()));
        }
        let Some(wired) = revisions.wires.get(&definition.version()) else {
            continue;
        };
        #[cfg(feature = "plugin-guest")]
        if require_unoccupied && wired.staged.as_ref().is_some_and(|staged| !staged.scope.has_committed()) {
            return Err(StreamRegistrationError::Occupied(definition.id().into()));
        }
        if require_unoccupied
            && wired
                .publisher
                .lock()
                .expect("stream publisher poisoned")
                .upgrade()
                .is_some_and(|lease| *lease.lock().expect("stream lease poisoned"))
        {
            return Err(StreamRegistrationError::Occupied(definition.id().into()));
        }
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::super::definition::Revisions;
    use super::*;
    use harmos_domain::schema::{
        Cardinality,
        graph::{Field, Kind, Schema},
    };
    use std::collections::HashMap;

    #[test]
    fn admission_rejects_native_other_owner_and_revision_collision() {
        let schema = Schema {
            representations: ::std::collections::BTreeMap::new(),
            root: 0,
            definitions: vec![vec![Field {
                name: "value".into(),
                tag: 1,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            }]],
        };
        let revision = harmos_domain::schema::stream_revision(&schema, 4).unwrap();
        let proposed = Definition::declared::<PublicationRow>("samples", schema.clone(), 4).unwrap().publication("owner");
        let mut revisions = Revisions::default();
        revisions.definitions.insert(revision, Contract::of(&Definition::declared::<u64>("samples", schema.clone(), 4).unwrap()));
        let mut windows = HashMap::from([(Arc::from("samples"), revisions)]);
        assert!(matches!(validate_existing(&windows, std::slice::from_ref(&proposed), false), Err(StreamRegistrationError::Native(_))));
        windows.get_mut("samples").unwrap().definitions.insert(
            revision,
            Contract::of(&Definition::declared::<PublicationRow>("samples", schema.clone(), 4).unwrap().publication("another")),
        );
        assert!(matches!(
            validate_existing(&windows, std::slice::from_ref(&proposed), false),
            Err(StreamRegistrationError::OwnerConflict(_))
        ));
        let mut changed = schema;
        changed.definitions[0][0].kind = Kind::String;
        windows
            .get_mut("samples")
            .unwrap()
            .definitions
            .insert(revision, Contract::of(&Definition::declared::<PublicationRow>("samples", changed, 4).unwrap().publication("owner")));
        assert!(matches!(validate_existing(&windows, &[proposed], false), Err(StreamRegistrationError::Collision(_))));
    }
}
