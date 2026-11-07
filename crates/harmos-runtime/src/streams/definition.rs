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

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock, Weak};
// Third Party Imports
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;
// Local Imports
use super::demand::Holding;
use super::window::{Row, Window};
use crate::streams::{BatchDefinition, Definition, StreamCatalog, StreamRow};
use crate::{Application, state::Position, storage::Cursor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(super) type Windows = HashMap<Arc<str>, Revisions>;

/* === Structs === */

#[derive(Debug, Default)]
pub(super) struct Revisions {
    pub(super) current: Option<u32>,
    pub(super) wires: HashMap<u32, Arc<Wired>>,
    pub(super) definitions: HashMap<u32, Contract>,
}

// Historical Collision Evidence Retains No Live Window Or Row Descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Contract {
    pub(super) owner: Option<Arc<str>>,
    row: Option<harmos_domain::Fingerprint>,
    window: usize,
}

pub(crate) struct Delivery {
    pub(crate) rows: Vec<(Cursor, Definition, Position, Row)>,
    pub(crate) reaches: Cursor,
    pub(crate) incomplete: Vec<Definition>,
}

#[derive(Debug)]
pub(crate) struct Wired {
    pub(crate) definition: Definition,
    pub(crate) window: Arc<Window>,
    pub(super) validation: Option<Arc<crate::schema::Validator>>,
    pub(super) publisher: Mutex<Weak<Mutex<bool>>>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) staged: Option<super::registration::Staged>,
}

#[derive(Debug)]
pub(crate) struct Table {
    pub(super) windows: RwLock<Windows>,
    pub(super) sequence: Arc<AtomicU64>,
    drained: AtomicU64,
    pub(super) pump: watch::Sender<()>,
    pub(super) visibility: Arc<RwLock<()>>,
    pub(super) publications: Arc<super::publication::Pool>,
    pub(super) ingest: Arc<super::ingest::Ingest>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) listeners: Mutex<BTreeMap<String, Arc<super::listeners::Listening>>>,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Refused {
    #[error("two stream definitions share the id \"{0}\"")]
    SharedId(Arc<str>),

    #[error("the stream \"{0}\" declares a window of zero rows")]
    EmptyWindow(Arc<str>),

    #[error("a stream consumer selected the undeclared stream \"{0}\"")]
    UndeclaredSelection(Arc<str>),
}

/* === Traits === */

pub trait Stream<A: Application>: crate::schema::Described + Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    const ID: &'static str;

    const WINDOW: usize;

    fn batch_definition() -> Option<BatchDefinition> {
        None
    }

    fn metadata() -> crate::Metadata {
        crate::Metadata::new(crate::Identifier::declared(Self::ID, "stream"), Self::ID, None)
    }

    fn row_metadata() -> Vec<crate::StreamField> {
        Vec::new()
    }

    fn definition() -> Definition {
        Definition::native::<Self>(Self::ID, Self::WINDOW)
            .batching(Self::batch_definition())
            .presentation(Self::metadata(), Self::row_metadata())
    }
}

/* === Implementations === */

impl<A, S> Stream<A> for S
where
    A: Application,
    S: StreamRow,
    A::Streams: StreamCatalog<S>,
{
    const ID: &'static str = S::ID;
    const WINDOW: usize = S::WINDOW;
    fn metadata() -> crate::Metadata {
        S::metadata()
    }

    fn row_metadata() -> Vec<crate::StreamField> {
        S::row_metadata()
    }

    fn batch_definition() -> Option<BatchDefinition> {
        S::batch_definition()
    }
}

impl Contract {
    pub(super) fn of(definition: &Definition) -> Self {
        Self {
            owner: definition.owner().map(Arc::from),
            row: definition.schema().map(|schema| {
                harmos_domain::schema::identity::fingerprint(schema.root().expect("validated row schema"), "harmos.postcard.v1")
                    .expect("validated canonical row schema")
            }),
            window: definition.window(),
        }
    }
}

impl Table {
    pub(crate) fn resuming(definitions: impl IntoIterator<Item = Definition>, floor: Position, resume: Cursor) -> Result<Self, Refused> {
        let (pump, _) = watch::channel(());
        let table = Self {
            windows: RwLock::new(HashMap::new()),
            sequence: Arc::new(AtomicU64::new(resume.0)),
            drained: AtomicU64::new(resume.0),
            pump,
            visibility: Arc::new(RwLock::new(())),
            publications: Arc::new(super::publication::Pool::new()),
            ingest: Arc::new(super::ingest::Ingest::new()),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            listeners: Mutex::new(BTreeMap::new()),
        };

        for definition in definitions {
            table.insert(definition, floor)?;
        }

        Ok(table)
    }

    pub(crate) fn insert(&self, definition: Definition, floor: Position) -> Result<Arc<Wired>, Refused> {
        if definition.window() == 0 {
            return Err(Refused::EmptyWindow(definition.id().into()));
        }

        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let mut windows = self.windows.write().expect("stream registry poisoned");
        if windows.contains_key(definition.id()) {
            return Err(Refused::SharedId(definition.id().into()));
        }

        let window = Arc::new(
            Window::wired(definition.window(), floor, Arc::clone(&self.sequence), self.pump.clone())
                .visible_with(self.visibility.clone())
                .batching(definition.batch()),
        );
        let wired = Arc::new(Wired {
            validation: None,
            definition,
            window,
            publisher: Mutex::new(Weak::new()),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            staged: None,
        });
        windows.insert(
            Arc::<str>::from(wired.definition.id()),
            Revisions {
                current: Some(wired.definition.version()),
                definitions: HashMap::from([(wired.definition.version(), Contract::of(&wired.definition))]),
                wires: HashMap::from([(wired.definition.version(), Arc::clone(&wired))]),
            },
        );

        Ok(wired)
    }

    pub(crate) fn wired(&self, id: &str, version: u32) -> Option<Arc<Wired>> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let wired =
            self.windows.read().expect("stream registry poisoned").get(id).and_then(|revisions| revisions.wires.get(&version)).cloned()?;
        wired.admitted()
    }

    pub(crate) fn declared(&self) -> Vec<Arc<Wired>> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        self.windows
            .read()
            .expect("stream registry poisoned")
            .values()
            .filter_map(|revisions| revisions.current.and_then(|current| revisions.wires.get(&current)).and_then(Wired::admitted))
            .collect()
    }

    pub(crate) fn current(&self, id: &str) -> Option<Arc<Wired>> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let windows = self.windows.read().expect("stream registry poisoned");
        let revisions = windows.get(id)?;
        revisions.wires.get(&revisions.current?).and_then(Wired::admitted)
    }

    // Retain Only Windows Still Needed By A Live Feed Or Staging.
    pub(super) fn retire(&self, windows: &mut Windows) {
        let taken = self.ingest.taken().min(self.drained.load(std::sync::atomic::Ordering::Acquire));
        for revisions in windows.values_mut() {
            revisions.wires.retain(|revision, wired| {
                let pending = {
                    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                    {
                        wired.staged.as_ref().is_some_and(|staged| !staged.scope.has_committed())
                    }
                    #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
                    {
                        false
                    }
                };
                revisions.current == Some(*revision) || pending || wired.window.latest_locked() > taken
            });
        }
    }

    pub(crate) fn hold(&self, selections: impl IntoIterator<Item = Vec<Definition>>) -> Result<Vec<Holding>, Refused> {
        let mut held = Vec::new();

        for selections in selections {
            let mut distinct = HashSet::new();
            for definition in selections {
                if !distinct.insert((Arc::<str>::from(definition.id()), definition.version())) {
                    continue;
                }
                let wired =
                    self.wired(definition.id(), definition.version()).ok_or(Refused::UndeclaredSelection(definition.id().into()))?;
                held.push(Holding::engage(Arc::clone(&wired.window)));
            }
        }

        Ok(held)
    }

    pub(crate) fn drained(&self, cursor: u64) {
        self.drained.store(cursor, std::sync::atomic::Ordering::Release);
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let mut windows = self.windows.write().expect("stream registry poisoned");
        self.retire(&mut windows);
    }

    pub(crate) fn delivery(&self) -> watch::Receiver<()> {
        self.pump.subscribe()
    }

    pub(crate) fn recorded_after(&self, resume: Cursor) -> Delivery {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        // Hold admission until the global cut is assembled. A new window
        // publishing below `reaches` must not be absent from this delivery.
        let mut windows = self.windows.write().expect("stream registry poisoned");
        self.retire(&mut windows);
        // Include every physical window in the cut. A shared generation may
        // commit while this read lock is held; its first rows must remain part
        // of the global cursor span even when catalog discovery began earlier.
        let reaches = windows
            .values()
            .flat_map(|revisions| revisions.wires.values())
            .map(|wired| wired.window.latest_locked())
            .max()
            .unwrap_or(resume.0)
            .max(resume.0);
        let mut rows = Vec::new();
        let mut incomplete = Vec::new();
        for wired in windows.values().flat_map(|revisions| revisions.wires.values()) {
            let (retained, lost) = wired.window.recorded_after_locked(resume.0, reaches);
            let Some(wired) = wired.admitted() else {
                continue;
            };
            rows.extend(retained.into_iter().map(|(cursor, position, row)| (Cursor(cursor), wired.definition.clone(), position, row)));
            if lost {
                incomplete.push(wired.definition.clone());
            }
        }
        rows.sort_unstable_by_key(|(cursor, ..)| *cursor);

        Delivery {
            rows,
            reaches: Cursor(reaches),
            incomplete,
        }
    }

    pub(crate) fn frontier(&self) -> Cursor {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        Cursor(self.sequence.load(std::sync::atomic::Ordering::Acquire))
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) fn can_advance(&self, count: u64) -> bool {
        self.sequence.load(std::sync::atomic::Ordering::Relaxed).checked_add(count).is_some()
    }

    pub(crate) fn wake(&self) {
        self.pump.send_modify(|()| {});
        for wired in self.declared() {
            wired.window.knock();
        }
    }
}

impl Wired {
    fn admitted(self: &Arc<Self>) -> Option<Arc<Self>> {
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if let Some(staged) = &self.staged
            && !staged.scope.has_committed()
        {
            return staged.previous.clone();
        }
        Some(self.clone())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(all(test, any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
mod tests {
    use super::*;

    #[test]
    fn retired_windows_release_after_feeds_and_readers_release_them() {
        let table = Table::resuming([], Position::ORIGIN, Cursor::ORIGIN).unwrap();
        let feed = table.ingest.engage(0);
        let mut weak = Vec::new();
        let mut readers = Vec::new();
        for index in 1..=64 {
            let definition = Definition::native::<u64>("samples", index);
            let revision = definition.version();
            let window = Arc::new(
                Window::wired(4, Position::ORIGIN, table.sequence.clone(), table.pump.clone()).visible_with(table.visibility.clone()),
            );
            window.publish(Position::ORIGIN, Arc::new(index as u64));
            weak.push(Arc::downgrade(&window));
            if index == 1 {
                readers.push(window.clone());
            }
            let wired = Arc::new(Wired {
                definition: definition.clone(),
                window,
                validation: None,
                publisher: Mutex::new(Weak::new()),
                staged: None,
            });
            {
                let mut windows = table.windows.write().unwrap();
                let revisions = windows.entry("samples".into()).or_default();
                revisions.current = Some(revision);
                revisions.definitions.insert(revision, Contract::of(&definition));
                revisions.wires.insert(revision, wired);
            }
            table.drained(u64::MAX);
        }
        assert_eq!(weak.iter().filter(|window| window.strong_count() > 0).count(), 64);
        feed.took(64);
        table.drained(64);
        assert_eq!(weak.iter().filter(|window| window.strong_count() > 0).count(), 2);
        readers.clear();
        assert_eq!(weak.iter().filter(|window| window.strong_count() > 0).count(), 1);
        assert_eq!(table.declared()[0].definition.version(), Definition::native::<u64>("samples", 64).version());
    }
}
