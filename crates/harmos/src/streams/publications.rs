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
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::definition::Windows;
use super::{Demand, Holding, Item, Query, StreamManager, Tier, Wired, window};
use crate::{Application, state::Position, streams::Definition};
use harmos_contract::inventory::publications;
use harmos_contract::{StreamDeclaration, artifact_subset};

/* === Constants === */
const MAX_PUBLICATION_BYTES: usize = 1024 * 1024;
const MAX_WINDOW_ROWS: usize = 65_536;

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
pub struct PluginStream {
    pub(super) wired: Arc<Wired>,
    pub(super) tier: Tier,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) pool: Arc<super::publication::Pool>,
}

pub struct PublicationLease {
    pub(super) stream: PluginStream,
    pub(super) active: Arc<Mutex<bool>>,
    pub(super) applied: watch::Receiver<Position>,
    pub(super) sequence: AtomicU64,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum StreamAdmissionError {
    #[error("`{0}` is not a portable plugin owner identity")]
    Owner(String),
    #[error(transparent)]
    Declaration(#[from] publications::Error),
    #[error("stream `{0}` is already owned by a live publication lease")]
    Occupied(String),
    #[error("stream `{0}` changes an admitted schema or retention contract without a new version")]
    Changed(String),
    #[error("stream `{0}` collides with a native catalog definition")]
    Native(String),
    #[error("a plugin stream window exceeds the 65536 row limit")]
    WindowLimit,
    #[error("stream bindings differ from the admitted plugin inventory")]
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
    #[error("the plugin activation has not committed")]
    NotReady,
    #[error(transparent)]
    Payload(#[from] crate::schema::PayloadError),
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> StreamManager<A> {
    pub async fn admit(
        &self,
        owner: &str,
        declaration: StreamDeclaration,
    ) -> Result<PublicationLease, StreamAdmissionError> {
        Ok(self.admit_all(owner, [declaration]).await?.remove(0))
    }

    pub fn validate_publications(
        &self,
        owner: &str,
        declarations: impl IntoIterator<Item = StreamDeclaration>,
    ) -> Result<(), StreamAdmissionError> {
        let definitions = publication_definitions(owner, declarations)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(StreamAdmissionError::Closed);
        }
        let _visibility = self
            .windows
            .visibility
            .read()
            .expect("stream visibility poisoned");
        let windows = self
            .windows
            .windows
            .read()
            .expect("stream registry poisoned");
        validate_existing(&windows, &definitions, false)
    }

    pub async fn admit_all(
        &self,
        owner: &str,
        declarations: impl IntoIterator<Item = StreamDeclaration>,
    ) -> Result<Vec<PublicationLease>, StreamAdmissionError> {
        let definitions = publication_definitions(owner, declarations)?;
        let validations = definitions
            .iter()
            .map(|definition| {
                crate::schema::Validator::from_schema(
                    definition
                        .schema()
                        .expect("plugin definitions own a row schema"),
                    &harmos_contract::config::SchemaSettings::default(),
                )
                .map_err(crate::schema::Error::from)
                .map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        {
            let _visibility = self
                .windows
                .visibility
                .read()
                .expect("stream visibility poisoned");
            let windows = self
                .windows
                .windows
                .read()
                .expect("stream registry poisoned");
            validate_existing(&windows, &definitions, true)?;
        }
        if self.closed.load(Ordering::Acquire) {
            return Err(StreamAdmissionError::Closed);
        }
        let mut floors = Vec::with_capacity(definitions.len());
        for definition in &definitions {
            floors.push(self.tier.seal(definition.clone(), Vec::new()).await);
        }

        let _visibility = self
            .windows
            .visibility
            .read()
            .expect("stream visibility poisoned");
        let mut windows = self
            .windows
            .windows
            .write()
            .expect("stream registry poisoned");
        if self.closed.load(Ordering::Acquire) {
            return Err(StreamAdmissionError::Closed);
        }
        validate_existing(&windows, &definitions, true)?;
        let mut leases = Vec::with_capacity(definitions.len());
        for ((definition, floor), validation) in
            definitions.into_iter().zip(floors).zip(validations)
        {
            let revisions = windows
                .entry(Arc::<str>::from(definition.id()))
                .or_default();
            // A presentation-only edit keeps the wire revision and retained rows.
            // Existing readers keep their immutable descriptor; new discovery sees
            // the replacement's authored metadata.
            if let Some(previous) = revisions.get(&definition.version())
                && (previous.is_staged()
                    || previous.definition.metadata() != definition.metadata()
                    || previous.definition.row_metadata() != definition.row_metadata())
            {
                revisions.insert(
                    definition.version(),
                    Arc::new(Wired {
                        validation: Some(Arc::clone(&validation)),
                        window: Arc::clone(&previous.window),
                        definition: definition.clone(),
                        publisher: Mutex::new(Weak::new()),
                        #[cfg(any(
                            feature = "plugin",
                            feature = "plugin-guest",
                            feature = "plugin-sidecar"
                        ))]
                        staged: None,
                    }),
                );
            }
            let wired = revisions
                .entry(definition.version())
                .or_insert_with(|| {
                    let wired = Arc::new(Wired {
                        validation: Some(Arc::clone(&validation)),
                        window: Arc::new(
                            window::Window::wired(
                                definition.window(),
                                floor,
                                Arc::clone(&self.windows.sequence),
                                self.windows.pump.clone(),
                            )
                            .visible_with(self.windows.visibility.clone()),
                        ),
                        definition,
                        publisher: Mutex::new(Weak::new()),
                        #[cfg(any(
                            feature = "plugin",
                            feature = "plugin-guest",
                            feature = "plugin-sidecar"
                        ))]
                        staged: None,
                    });
                    if floor > Position::ORIGIN && floor <= *self.applied.borrow() {
                        wired.window.stands_on(floor);
                    }
                    wired
                })
                .clone();
            let active = Arc::new(Mutex::new(true));
            *wired.publisher.lock().expect("stream publisher poisoned") = Arc::downgrade(&active);
            leases.push(PublicationLease {
                stream: PluginStream {
                    wired,
                    tier: self.tier.clone(),
                    closed: Arc::clone(&self.closed),
                    pool: self.windows.publications.clone(),
                },
                active,
                applied: self.applied.clone(),
                sequence: AtomicU64::new(0),
            });
        }
        Ok(leases)
    }

    pub fn publication(&self, id: &str, version: u32) -> Option<PluginStream> {
        let wired = self.windows.wired(id, version)?;
        wired.definition.owner()?;

        Some(PluginStream {
            wired,
            tier: self.tier.clone(),
            closed: Arc::clone(&self.closed),
            pool: self.windows.publications.clone(),
        })
    }

    pub fn definitions(&self) -> Vec<Definition> {
        let mut definitions: Vec<_> = self
            .windows
            .declared()
            .into_iter()
            .map(|wired| wired.definition.clone())
            .collect();
        definitions.sort_by(|a, b| (a.id(), a.version()).cmp(&(b.id(), b.version())));
        definitions
    }
}

impl PublicationLease {
    pub fn stream(&self) -> &PluginStream {
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

    pub fn publish_row(
        &self,
        row: PublicationRow,
        missed: u64,
    ) -> Result<(), StreamPublicationError> {
        self.admit(row, None, missed).map(|_| ())
    }

    pub fn admit_row(
        &self,
        row: PublicationRow,
        wire_sequence: u64,
        missed: u64,
    ) -> Result<super::ManagedPublication, StreamPublicationError> {
        self.admit(row, Some(wire_sequence), missed)
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn stage_row(
        &self,
        row: PublicationRow,
        wire_sequence: u64,
        missed: u64,
    ) -> Result<super::StagedPublication, StreamPublicationError> {
        self.stage_sequence(vec![row], Some(wire_sequence), missed, None)
            .single()
    }

    // An ordered batch takes contiguous sequences from `first_wire_sequence`;
    // `missed` applies before the first row only.
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn stage_rows(
        &self,
        rows: Vec<PublicationRow>,
        first_wire_sequence: u64,
        missed: u64,
    ) -> super::StagedRows {
        self.stage_sequence(rows, Some(first_wire_sequence), missed, None)
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn stage_derived_row(
        &self,
        row: PublicationRow,
        missed: u64,
    ) -> Result<super::StagedPublication, StreamPublicationError> {
        self.stage_sequence(vec![row], None, missed, None).single()
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn stage_rows_in(
        &self,
        context: &crate::operations::Context<'_>,
        rows: Vec<PublicationRow>,
        first_wire_sequence: u64,
        missed: u64,
    ) -> super::StagedRows {
        self.stage_sequence(rows, Some(first_wire_sequence), missed, Some(context))
    }

    // One visibility fence and one scope phase cover the whole batch, so no
    // reader or commit observes it half staged.
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    fn stage_sequence(
        &self,
        rows: Vec<PublicationRow>,
        wire_sequence: Option<u64>,
        missed: u64,
        context: Option<&crate::operations::Context<'_>>,
    ) -> super::StagedRows {
        let Some(staged) = self.stream.wired.staged.as_ref() else {
            return super::StagedRows::refused(Vec::new(), StreamPublicationError::NotReady);
        };
        let _visibility = self
            .stream
            .wired
            .window
            .visibility
            .write()
            .expect("stream visibility poisoned");
        let mut evicted = Vec::new();
        let prepared = staged.scope.prepare_publication_in(context, |ready| {
            let activation = staged
                .activation
                .upgrade()
                .ok_or(StreamPublicationError::Revoked)?;
            let mut receipts = Vec::with_capacity(rows.len());
            for (offset, row) in (0_u64..).zip(rows) {
                let sequence = match wire_sequence.map(|first| first.checked_add(offset)) {
                    Some(None) => {
                        return Ok((receipts, ready, Some(StreamPublicationError::Sequence)));
                    }
                    sequence => sequence.flatten(),
                };
                let missed = if offset == 0 { missed } else { 0 };
                let (publication, dropped) = match self.admit_locked(row, sequence, missed, ready) {
                    Ok(admitted) => admitted,
                    Err(error) => return Ok((receipts, ready, Some(error))),
                };
                evicted.extend(dropped);
                receipts.push(if ready {
                    let (_, committed) = watch::channel(true);
                    super::StagedPublication {
                        publication,
                        committed,
                    }
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
        &self,
        row: PublicationRow,
        sequence: Option<u64>,
        missed: u64,
    ) -> Result<super::ManagedPublication, StreamPublicationError> {
        let _visibility = self
            .stream
            .wired
            .window
            .visibility
            .write()
            .expect("stream visibility poisoned");
        let result = {
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
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
            #[cfg(not(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            )))]
            {
                self.admit_locked(row, sequence, missed, true)
            }
        };
        drop(_visibility);
        let (publication, evicted) = result?;
        drop(evicted);
        self.stream.wired.window.knock();
        self.stream.wired.window.wake_delivery();
        Ok(publication)
    }

    // Visibility and original scope phase precede writer and row-state locks.
    fn admit_locked(
        &self,
        row: PublicationRow,
        sequence: Option<u64>,
        missed: u64,
        live: bool,
    ) -> Result<(super::ManagedPublication, Option<super::window::Row>), StreamPublicationError>
    {
        let active = self.active.lock().expect("stream lease poisoned");
        if !*active || self.stream.closed.load(Ordering::Acquire) {
            return Err(StreamPublicationError::Revoked);
        }
        if row.payload.len() > MAX_PUBLICATION_BYTES {
            return Err(StreamPublicationError::Oversized);
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
        self.stream
            .wired
            .validation
            .as_ref()
            .expect("compiled plugin schema")
            .validate(&row.payload)?;
        let generation = {
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            {
                self.stream
                    .wired
                    .staged
                    .as_ref()
                    .map(|staged| staged.scope.generation().get())
            }
            #[cfg(not(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            )))]
            {
                None
            }
        };
        if live
            && !self.stream.wired.window.can_advance(
                missed
                    .checked_add(1)
                    .ok_or(StreamPublicationError::Sequence)?,
            )
        {
            return Err(StreamPublicationError::Sequence);
        }
        let publication = self.stream.pool.retain(
            &self.stream.wired.definition,
            generation,
            row,
            next,
            missed,
            live.then_some(self.stream.wired.window.as_ref()),
        )?;
        let evicted = if live {
            self.stream.wired.window.append_locked(
                *self.applied.borrow(),
                Arc::new(publication.clone()),
                missed,
            )?
        } else {
            None
        };
        self.sequence.store(next, Ordering::Relaxed);
        Ok((publication, evicted))
    }

    pub fn revoke(&self) {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some(staged) = &self.stream.wired.staged {
            staged.scope.unbind_streams();
        }
        *self.active.lock().expect("stream lease poisoned") = false;
    }
}

impl Drop for PublicationLease {
    fn drop(&mut self) {
        self.revoke();
    }
}

impl PluginStream {
    pub fn definition(&self) -> &Definition {
        &self.wired.definition
    }

    pub fn query(&self, range: impl RangeBounds<Position>) -> Query<PublicationRow> {
        Query {
            window: Arc::clone(&self.wired.window),
            definition: self.wired.definition.clone(),
            tier: self.tier.clone(),
            start: range.start_bound().cloned(),
            end: range.end_bound().cloned(),
            as_of: None,
            partition: None,
            retained_only: false,
            marker: PhantomData,
        }
    }

    pub fn watch(
        &self,
    ) -> impl futures_core::Stream<Item = Item<PublicationRow>> + Send + Unpin + use<> {
        let window = Arc::clone(&self.wired.window);
        let holding = Holding::engage(Arc::clone(&window));
        window::following(window, Arc::clone(&self.closed), holding)
    }

    pub fn watch_managed(
        &self,
    ) -> impl futures_core::Stream<Item = Item<super::ManagedPublication>> + Send + Unpin + use<>
    {
        let window = Arc::clone(&self.wired.window);
        let holding = Holding::engage(Arc::clone(&window));
        window::following(window, Arc::clone(&self.closed), holding)
    }

    pub fn demand(&self) -> Demand {
        Demand::of(&self.wired.window)
    }
}

impl Wired {
    fn is_staged(&self) -> bool {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        {
            self.staged.is_some()
        }
        #[cfg(not(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        )))]
        {
            false
        }
    }
}

/* === Public Functions === */
pub(super) fn publication_definitions(
    owner: &str,
    declarations: impl IntoIterator<Item = StreamDeclaration>,
) -> Result<Vec<Definition>, StreamAdmissionError> {
    if !artifact_subset(owner) {
        return Err(StreamAdmissionError::Owner(owner.into()));
    }
    let mut ids = BTreeSet::new();
    declarations
        .into_iter()
        .map(|declaration| {
            publications::validate(&declaration)?;
            if declaration.window > MAX_WINDOW_ROWS {
                return Err(StreamAdmissionError::WindowLimit);
            }
            if !ids.insert(declaration.id.clone()) {
                return Err(publications::Error::DuplicateStream { id: declaration.id }.into());
            }
            let definition = Definition::declared::<PublicationRow>(
                declaration.id,
                declaration.schema,
                declaration.window,
            );
            let definition = definition
                .presentation(declaration.metadata, declaration.row_metadata)
                .publication(owner, declaration.row);
            Ok(definition)
        })
        .collect()
}

pub(super) fn validate_existing(
    windows: &Windows,
    definitions: &[Definition],
    require_unoccupied: bool,
) -> Result<(), StreamAdmissionError> {
    for definition in definitions {
        let Some(revisions) = windows.get(definition.id()) else {
            continue;
        };
        if revisions
            .values()
            .any(|wired| wired.definition.owner().is_none())
        {
            return Err(StreamAdmissionError::Native(definition.id().into()));
        }
        if revisions
            .values()
            .any(|wired| wired.definition.owner() != definition.owner())
        {
            return Err(StreamAdmissionError::Changed(definition.id().into()));
        }
        let Some(wired) = revisions.get(&definition.version()) else {
            continue;
        };
        if wired.definition.owner() != definition.owner()
            || wired.definition.schema() != definition.schema()
            || wired.definition.window() != definition.window()
        {
            return Err(StreamAdmissionError::Changed(definition.id().into()));
        }
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if require_unoccupied
            && wired
                .staged
                .as_ref()
                .is_some_and(|staged| !staged.scope.has_committed())
        {
            return Err(StreamAdmissionError::Occupied(definition.id().into()));
        }
        if require_unoccupied
            && wired
                .publisher
                .lock()
                .expect("stream publisher poisoned")
                .upgrade()
                .is_some_and(|lease| *lease.lock().expect("stream lease poisoned"))
        {
            return Err(StreamAdmissionError::Occupied(definition.id().into()));
        }
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
