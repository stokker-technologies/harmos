/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded Depth-First Enumeration With Retained Entry Ownership

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::{self, ReadDir};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::task::JoinHandle;
// Local Imports
use super::{DirectoryCompletion, DirectoryEntry, Selection, join_error, relative, retained_item, selection};
use crate::capacity::{Held, Pool, Refusal, Reservation};
use crate::manager::{ENTRY_OVERHEAD, Inner};
use crate::{Error, ReadBatch, Result};
use harmos_domain::model::storage::DirectorySelection;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Scanned = (DirectoryEntry, Reservation);
type Pending = JoinHandle<Result<Option<Scanned>>>;

/* === Structs === */

pub struct DirectoryReader {
    context: crate::execution::Context,
    pub(super) inner: Arc<Inner>,
    state: Arc<Mutex<Scan>>,
    pending: Option<Pending>,
    cancelled: Arc<AtomicBool>,
}

struct Scan {
    inner: Arc<Inner>,
    held: Held,
    selection: Arc<Selection>,
    stack: Vec<Frame>,
    visited: usize,
    entries: u64,
    complete: bool,
    cancelled: Arc<AtomicBool>,
}

struct Frame {
    directory: ReadDir,
    _path: PathBuf,
    _memory: Reservation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl DirectoryReader {
    pub(super) async fn new(inner: Arc<Inner>, value: DirectorySelection) -> Result<Self> {
        let held = Held::default();
        let selected = selection(&inner, value, &held).await?;
        Self::selected(inner, selected, held).await
    }

    pub(super) async fn selected(inner: Arc<Inner>, selection: Arc<Selection>, held: Held) -> Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let scan = Scan {
            inner: Arc::clone(&inner),
            held: held.clone(),
            selection,
            stack: Vec::new(),
            visited: 0,
            entries: 0,
            complete: false,
            cancelled: Arc::clone(&cancelled),
        };
        let scan = {
            let context = crate::execution::Context::current();
            tokio::task::spawn_blocking(move || {
                context.blocking(|| {
                    let mut scan = scan;
                    let selection = Arc::clone(&scan.selection);
                    scan.enter(&selection.value.root)?;
                    Ok::<_, Error>(scan)
                })
            })
        }
        .await
        .map_err(join_error)??;
        Ok(Self {
            context: crate::execution::Context::current(),
            inner,
            state: Arc::new(Mutex::new(scan)),
            pending: None,
            cancelled,
        })
    }

    pub async fn next(&mut self) -> Result<Option<ReadBatch<DirectoryEntry>>> {
        self.next_value().await?.map(|(value, memory)| retained_item(&self.inner, value, memory)).transpose()
    }

    pub fn completion(&self) -> Result<DirectoryCompletion> {
        if self.pending.is_some() {
            return Err(Error::NotReady { domain: "directory completion" });
        }
        let scan = self.state.lock().unwrap();
        if !scan.complete {
            return Err(Error::NotReady { domain: "directory completion" });
        }
        Ok(DirectoryCompletion { entries: scan.entries })
    }

    pub(super) async fn next_value(&mut self) -> Result<Option<Scanned>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::debug_span!("storage.directory.page", outcome = "cancelled"));
        context
            .result(span, async {
                if self.pending.is_none() {
                    let state = Arc::clone(&self.state);
                    let context = self.context.clone();
                    self.pending = Some(tokio::task::spawn_blocking(move || context.blocking(|| state.lock().unwrap().next())));
                }
                let result = self.pending.as_mut().unwrap().await;
                self.pending = None;
                result.map_err(join_error)?
            })
            .await
    }
}

impl Drop for DirectoryReader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl Scan {
    fn enter(&mut self, path: &Path) -> Result<()> {
        let bytes = path.as_os_str().len().saturating_mul(6).saturating_add(std::mem::size_of::<Frame>() * 2 + ENTRY_OVERHEAD);
        let memory = Reservation::try_new(&self.inner, Pool::Read, &self.held, bytes).map_err(Refusal::into_error)?;
        let path = crate::local::validate_local_path(path)?;
        let directory = fs::read_dir(&path).map_err(crate::local::io_error)?;
        self.stack.push(Frame { directory, _path: path, _memory: memory });
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Scanned>> {
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(Error::Closing { domain: "directory scan" });
            }
            let Some(frame) = self.stack.last_mut() else {
                self.complete = true;
                return Ok(None);
            };
            let Some(entry) = frame.directory.next() else {
                self.stack.pop();
                continue;
            };
            let entry = entry.map_err(crate::local::io_error)?;
            self.visited = self.visited.checked_add(1).ok_or(Error::ResourceExhausted { resource: "directory entries" })?;
            if self.visited > self.inner.settings.filesystem.max_entries.get() {
                return Err(Error::ResourceExhausted { resource: "directory entries" });
            }
            let name = entry.file_name();
            let bytes = frame._path.capacity().saturating_add(name.capacity()).saturating_mul(3).saturating_add(ENTRY_OVERHEAD);
            let memory = Reservation::try_new(&self.inner, Pool::Read, &self.held, bytes).map_err(Refusal::into_error)?;
            let path = frame._path.join(name);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(crate::local::io_error(error)),
            };
            if crate::local::linked(&metadata) {
                continue;
            }
            if metadata.is_dir() && self.selection.value.recursive {
                self.enter(&path)?;
            }
            let Some(path) = relative(&self.selection.value.root, &path, self.selection.value.recursive)? else { continue };
            if !super::selection::selected(&path, &self.selection.value.rules) {
                continue;
            }
            self.entries += 1;
            let entry = DirectoryEntry {
                path,
                directory: metadata.is_dir(),
                length: if metadata.is_file() { metadata.len() } else { 0 },
            };
            return Ok(Some((entry, memory)));
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
