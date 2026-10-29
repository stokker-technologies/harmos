/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock, Weak};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use super::demand::Holding;
use super::window::{Row, Window};
use crate::streams::{BatchDefinition, Definition, StreamCatalog, StreamRow};
use crate::{Application, state::Position, storage::Cursor};

pub(super) type Windows = HashMap<Arc<str>, HashMap<u32, Arc<Wired>>>;

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
    pub(super) pump: watch::Sender<()>,
    pub(super) visibility: Arc<RwLock<()>>,
    pub(super) publications: Arc<super::publication::Pool>,
    pub(super) ingest: Arc<super::ingest::Ingest>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) listeners: Mutex<BTreeMap<String, Arc<super::listeners::Listening>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Refused {
    #[error("two stream definitions share the id \"{0}\"")]
    SharedId(Arc<str>),

    #[error("the stream \"{0}\" declares a window of zero rows")]
    EmptyWindow(Arc<str>),

    #[error("a stream consumer selected the undeclared stream \"{0}\"")]
    UndeclaredSelection(Arc<str>),
}

pub trait Stream<A: Application>: Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    const ID: &'static str;

    const VERSION: u32;

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
        Definition::declared::<Self>(Self::ID, Self::VERSION, Self::WINDOW)
            .batching(Self::batch_definition())
            .presentation(Self::metadata(), Self::row_metadata())
    }
}

impl<A, S> Stream<A> for S
where
    A: Application,
    S: StreamRow,
    A::Streams: StreamCatalog<S>,
{
    const ID: &'static str = S::ID;
    const VERSION: u32 = S::VERSION;
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

impl Table {
    pub(crate) fn resuming(definitions: impl IntoIterator<Item = Definition>, floor: Position, resume: Cursor) -> Result<Self, Refused> {
        let (pump, _) = watch::channel(());
        let table = Self {
            windows: RwLock::new(HashMap::new()),
            sequence: Arc::new(AtomicU64::new(resume.0)),
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
        windows.insert(Arc::<str>::from(wired.definition.id()), HashMap::from([(wired.definition.version(), Arc::clone(&wired))]));

        Ok(wired)
    }

    pub(crate) fn wired(&self, id: &str, version: u32) -> Option<Arc<Wired>> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let wired =
            self.windows.read().expect("stream registry poisoned").get(id).and_then(|revisions| revisions.get(&version)).cloned()?;
        wired.admitted()
    }

    pub(crate) fn declared(&self) -> Vec<Arc<Wired>> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        visible(
            self.windows
                .read()
                .expect("stream registry poisoned")
                .values()
                .flat_map(|revisions| revisions.values())
                .cloned()
                .collect::<Vec<_>>(),
        )
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

    pub(crate) fn delivery(&self) -> watch::Receiver<()> {
        self.pump.subscribe()
    }

    pub(crate) fn recorded_after(&self, resume: Cursor) -> Delivery {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        // Hold admission until the global cut is assembled. A new window
        // publishing below `reaches` must not be absent from this delivery.
        let windows = self.windows.read().expect("stream registry poisoned");
        // Include every physical window in the cut. A shared generation may
        // commit while this read lock is held; its first rows must remain part
        // of the global cursor span even when catalog discovery began earlier.
        let reaches = windows
            .values()
            .flat_map(|revisions| revisions.values())
            .map(|wired| wired.window.latest_locked())
            .max()
            .unwrap_or(resume.0)
            .max(resume.0);
        let mut rows = Vec::new();
        let mut incomplete = Vec::new();
        for wired in windows.values().flat_map(|revisions| revisions.values()) {
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
        let windows = self.windows.read().expect("stream registry poisoned");
        Cursor(windows.values().flat_map(|revisions| revisions.values()).map(|wired| wired.window.latest_locked()).max().unwrap_or(0))
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

fn visible(wires: impl IntoIterator<Item = Arc<Wired>>) -> Vec<Arc<Wired>> {
    let wires = wires.into_iter();
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    let wires = {
        let mut commits = std::collections::BTreeMap::new();
        wires.filter_map(move |wired| {
            if let Some(staged) = &wired.staged {
                let committed = *commits.entry(staged.scope.generation()).or_insert_with(|| staged.scope.has_committed());
                if !committed {
                    return staged.previous.clone();
                }
            }
            Some(wired)
        })
    };
    wires.collect()
}
