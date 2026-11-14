/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Unary File Transfer Handlers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::requests::{QueryOutput, Transform};

use super::requests::required;

use super::snapshot::{SavedSnapshots, SnapshotRead};
use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_domain::model::storage::query::{Query, TimeMerge};
use harmos_protocol::model;
use harmos_query::QueryStep;
use harmos_query::columnar::Cancellation;
use harmos_storage::QueryReader;
use harmos_storage::StorageManager;
use harmos_storage::query::RowSlice;
use harmos_storage::query::Source;
use prost::Message;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

/* === Structs === */

#[derive(Clone)]
pub(super) struct Page {
    pub(super) rows: Vec<QueryOutput>,
    pub(super) resolved_at: model::ReadCut,
    pub(super) next_page_token: String,
}

pub(super) struct Queries {
    maximum: usize,
    state: Mutex<Owners>,
}
#[derive(Default)]
struct Owners {
    entries: BTreeMap<Vec<u8>, Arc<tokio::sync::Mutex<Session>>>,
    order: VecDeque<Vec<u8>>,
}
struct Session {
    read: SnapshotRead,
    reader: Option<QueryReader>,
    sequence: u64,
    cached: Option<(u64, Bounds, Page)>,
    inflight: bool,
    failed: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Bounds {
    rows: NonZeroUsize,
    bytes: NonZeroUsize,
}

/* === Implementations === */

impl Queries {
    pub(super) fn new(maximum: NonZeroUsize) -> Self {
        Self {
            maximum: maximum.get(),
            state: Mutex::new(Owners::default()),
        }
    }
    pub(super) fn evict_idle(&self, pinned: Option<&BTreeSet<Vec<u8>>>) -> bool {
        let mut state = self.state.lock().unwrap();
        let selected = state.order.iter().position(|id| {
            state.entries.get(id).is_some_and(|session| {
                Arc::strong_count(session) == 1
                    && session.try_lock().is_ok_and(|session| pinned.is_none_or(|pinned| !pinned.contains(session.read.identity())))
            })
        });
        let Some(index) = selected else { return false };
        let id = state.order.remove(index).unwrap();
        let retired = state.entries.remove(&id);
        drop(state);
        drop(retired);
        true
    }
    fn prepare(&self) -> crate::Result<()> {
        while self.state.lock().unwrap().entries.len() >= self.maximum {
            if !self.evict_idle(None) {
                return Err(exhausted().into());
            }
        }
        Ok(())
    }
    fn insert(&self, reader: QueryReader, read: SnapshotRead) -> crate::Result<(Vec<u8>, Arc<tokio::sync::Mutex<Session>>)> {
        self.prepare()?;
        let mut id = vec![0; 32];
        let mut state = self.state.lock().unwrap();
        if state.entries.len() >= self.maximum {
            return Err(exhausted().into());
        }
        loop {
            getrandom::fill(&mut id).map_err(|_| exhausted())?;
            if !state.entries.contains_key(&id) {
                break;
            }
        }
        let session = Arc::new(tokio::sync::Mutex::new(Session {
            read,
            reader: Some(reader),
            sequence: 0,
            cached: None,
            inflight: false,
            failed: false,
        }));
        state.order.push_back(id.clone());
        state.entries.insert(id.clone(), session.clone());
        Ok((id, session))
    }
    fn get(&self, id: &[u8]) -> crate::Result<Arc<tokio::sync::Mutex<Session>>> {
        self.state.lock().unwrap().entries.get(id).cloned().ok_or_else(|| expired().into())
    }
}

/* === Public Functions === */

pub(super) async fn list(
    context: &ExecutionContext, source: Option<Source>, at: Option<model::ReadCut>, transforms: Vec<Transform>, merge: Option<TimeMerge>,
    page: Option<model::PageRequest>,
) -> Result<Page, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;

    let cancel = Cancellation::default();
    super::admission::run(
        context,
        async {
            let mut plan = b"storage.rows:".to_vec();
            plan.extend(super::admission::plan(&(source.clone(), transforms.clone(), merge.clone()))?);
            let source: Source = required(source)?;
            source.validate()?;

            let page = page.unwrap_or_default();
            let maximum = if page.max_bytes == 0 {
                storage.settings().response.batch_bytes
            } else {
                NonZeroUsize::new(usize::try_from(page.max_bytes).map_err(|_| exhausted())?)
                    .ok_or_else(exhausted)?
                    .min(storage.settings().response.batch_bytes)
            };
            let rows = if page.page_size == 0 {
                storage.settings().response.batch_rows
            } else {
                NonZeroUsize::new(page.page_size as usize).unwrap().min(storage.settings().response.batch_rows)
            };
            let bounds = Bounds { rows, bytes: maximum };
            let (identity, sequence, session) = if page.page_token.is_empty() {
                snapshots.queries.prepare()?;
                let read = snapshots.resolve(&storage, at.clone()).await?;

                let query = Query {
                    merge,
                    at: read.at().clone(),
                    source,
                    transforms,
                    slice: RowSlice::default(),
                };
                let reader = storage.query_snapshot(query, read.snapshot()).await?;
                let (identity, session) = snapshots.queries.insert(reader, read)?;
                (identity, 0, session)
            } else {
                let (identity, sequence) = snapshots.decode_page(&page.page_token, &plan)?;
                let session = snapshots.queries.get(&identity)?;
                (identity, sequence, session)
            };
            let mut session = session.lock().await;
            if !snapshots.contains(&session.read)
                || at.as_ref().is_some_and(|at| at != session.read.cut())
                || session.failed
                || session.inflight
            {
                return Err(expired().into());
            }
            if let Some((prior, original_bounds, response)) = &session.cached
                && *prior == sequence
            {
                if *original_bounds != bounds {
                    return Err(expired().into());
                }
                return Ok(response.clone());
            }
            if sequence != session.sequence {
                return Err(expired().into());
            }
            let cut = session.read.cut().clone();
            // Account For The Complete Cut, Token And Nested IPC Envelope Before Delivery.
            let overhead = field(cut.encoded_len()) + field(208) + 1 + prost::length_delimiter_len(maximum.get());
            let bytes = NonZeroUsize::new(maximum.get().saturating_sub(overhead)).ok_or_else(exhausted)?;
            let reader = session.reader.as_mut().ok_or_else(expired)?;
            reader.configure_page(rows, bytes);
            session.inflight = true;
            let step = session.reader.as_mut().unwrap().step().await;
            session.reader.as_mut().unwrap().suspend_page();
            session.inflight = false;
            let step = match step {
                Ok(step) => step,
                Err(error) => {
                    session.failed = true;
                    session.reader = None;
                    return Err(error.into());
                }
            };
            let next = sequence.checked_add(1).ok_or_else(expired)?;
            let (rows, next_page_token) = match step {
                QueryStep::Output(output) => (vec![output.output()], snapshots.token(&identity, &plan, next)?),
                QueryStep::Progress => (Vec::new(), snapshots.token(&identity, &plan, next)?),
                QueryStep::Complete => {
                    session.reader = None;
                    (Vec::new(), String::new())
                }
            };
            let encoded = rows.iter().map(|row| field(row.memory_size())).sum::<usize>()
                + field(cut.encoded_len())
                + if next_page_token.is_empty() { 0 } else { field(next_page_token.len()) };
            if encoded > maximum.get() {
                session.failed = true;
                session.reader = None;
                return Err(exhausted().into());
            }

            let response = Page { rows, resolved_at: cut, next_page_token };
            session.sequence = next;
            session.cached = Some((sequence, bounds, response.clone()));
            Ok(response)
        },
        cancel.clone(),
    )
    .await
}

/* === Private Functions === */

fn field(bytes: usize) -> usize {
    1 + prost::length_delimiter_len(bytes) + bytes
}
fn exhausted() -> harmos_storage::Error {
    harmos_storage::Error::ResourceExhausted {
        resource: "ListRows complete page or retained query capacity",
    }
}
fn expired() -> harmos_storage::Error {
    harmos_storage::Error::InvalidInput {
        field: "storage continuation",
        reason: "ListRows cut, token or query expired, changed or was interrupted; restart the read",
    }
}
