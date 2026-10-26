/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Subscription Precedes Enumeration; Overflow Requires An Explicit Rescan

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{Notify, mpsc};
// Local Imports
use super::{DirectoryEntry, DirectoryEvent, DirectoryReader, Selection, item, relative, retained_item, selection};
use crate::capacity::{Held, Pool, Refusal, Reservation};
use crate::manager::{ENTRY_OVERHEAD, Inner};
use crate::{Error, ReadBatch, Result};
use harmos_domain::model::storage::DirectorySelection;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct DirectoryWatcher {
    context: crate::execution::Context,
    inner: Arc<Inner>,
    held: Held,
    selection: Arc<Selection>,
    initial: Option<DirectoryReader>,
    initial_complete: bool,
    initial_marker: ReadBatch<DirectoryEvent>,
    rescan: ReadBatch<DirectoryEvent>,
    incoming: mpsc::Receiver<Queued>,
    ready: VecDeque<ReadBatch<DirectoryEvent>>,
    overflow: Arc<AtomicBool>,
    changed: Arc<Notify>,
    watcher: Option<RecommendedWatcher>,
    memory: Option<Reservation>,
}

struct Queued {
    event: Event,
    _memory: Reservation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl DirectoryWatcher {
    pub(super) async fn new(inner: Arc<Inner>, value: DirectorySelection, include_existing: bool) -> Result<Self> {
        let held = Held::default();
        let selected = selection(&inner, value, &held).await?;
        let initial_marker = item(&inner, &held, DirectoryEvent::InitialScanComplete)?;
        let rescan = item(&inner, &held, DirectoryEvent::RescanRequired)?;
        let settings = inner.settings.filesystem;
        // Tokio Allocates Queue Slots In Blocks; Include A Partial Block.
        let slots = settings.event_capacity.get().saturating_add(32);
        let bytes = slots.saturating_mul(std::mem::size_of::<Queued>() + 64).saturating_add(ENTRY_OVERHEAD);
        let memory = Reservation::try_new(&inner, Pool::Read, &held, bytes).map_err(Refusal::into_error)?;

        let (outgoing, incoming) = mpsc::channel(settings.event_capacity.get());
        let overflow = Arc::new(AtomicBool::new(false));
        let changed = Arc::new(Notify::new());
        let callback_inner = Arc::clone(&inner);
        let callback_held = held.clone();
        let callback_overflow = Arc::clone(&overflow);
        let callback_changed = Arc::clone(&changed);

        let span = tracing::debug_span!("storage.directory.subscription", outcome = "cancelled");
        let context = crate::execution::Context::with_span(span);
        let callback_context = context.clone();
        let handler = move |result: notify::Result<Event>| {
            callback_context.sync(|| {
                let Ok(event) = result else {
                    signal(&callback_overflow, &callback_changed);
                    return;
                };
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                if event.paths.is_empty() || event.paths.len() > settings.event_capacity.get() {
                    signal(&callback_overflow, &callback_changed);
                    return;
                }
                let paths = event.paths.iter().fold(0_usize, |bytes, path| bytes.saturating_add(path.capacity()));
                let bytes = paths
                    .saturating_mul(3)
                    .saturating_add(event.paths.capacity().saturating_mul(std::mem::size_of::<std::path::PathBuf>() * 3))
                    .saturating_add(ENTRY_OVERHEAD);
                let Ok(memory) = Reservation::try_new(&callback_inner, Pool::Read, &callback_held, bytes) else {
                    signal(&callback_overflow, &callback_changed);
                    return;
                };
                deliver(&outgoing, Queued { event, _memory: memory }, &callback_overflow, &callback_changed);
            })
        };
        let configuration = notify::Config::default().with_poll_interval(settings.poll_interval).with_follow_symlinks(false);
        let mut watcher = RecommendedWatcher::new(handler, configuration).map_err(watch_error)?;
        let mode = if selected.value.recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        watcher.watch(&selected.value.root, mode).map_err(watch_error)?;
        let initial = if include_existing {
            Some(DirectoryReader::selected(Arc::clone(&inner), Arc::clone(&selected), held.clone()).await?)
        } else {
            None
        };
        Ok(Self {
            context,
            inner,
            held,
            selection: selected,
            initial,
            initial_complete: false,
            initial_marker,
            rescan,
            incoming,
            ready: VecDeque::new(),
            overflow,
            changed,
            watcher: Some(watcher),
            memory: Some(memory),
        })
    }

    pub async fn next(&mut self) -> Result<Option<ReadBatch<DirectoryEvent>>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.directory.watch.page", outcome = "cancelled"));
        context
            .result(span, async {
                let mut lifecycle = self.inner.changed.subscribe();
                loop {
                    let closing = {
                        let state = self.inner.state.lock().unwrap();
                        state.status.closing || state.status.closed
                    };
                    if closing || self.watcher.is_none() {
                        self.stop();
                        return Ok(None);
                    }
                    if let Some(reader) = &mut self.initial {
                        let next = tokio::select! {
                            next = reader.next_value() => Some(next),
                            _ = lifecycle.changed() => None,
                        };
                        let Some(next) = next else { continue };
                        if let Some((entry, memory)) = next? {
                            return retained_item(&self.inner, DirectoryEvent::Existing(entry), memory).map(Some);
                        }
                        self.initial = None;
                    }
                    if !self.initial_complete {
                        self.initial_complete = true;
                        return Ok(Some(self.initial_marker.clone()));
                    }
                    if let Some(item) = self.ready.pop_front() {
                        return Ok(Some(item));
                    }

                    let changed = Arc::clone(&self.changed);
                    let notified = changed.notified();
                    if self.overflow.swap(false, Ordering::AcqRel) {
                        return Ok(Some(self.rescan()));
                    }
                    let queued = tokio::select! {
                        value = self.incoming.recv() => value,
                        _ = notified => continue,
                        _ = lifecycle.changed() => continue,
                    };
                    let Some(queued) = queued else { return Ok(None) };
                    if queued.event.paths.len() > self.inner.settings.filesystem.event_capacity.get() {
                        return Ok(Some(self.rescan()));
                    }
                    let events = events(&self.selection.value, queued.event)?;
                    if events.iter().any(|event| matches!(event, DirectoryEvent::RescanRequired)) {
                        return Ok(Some(self.rescan()));
                    }
                    for event in events {
                        // One Native Event Can Carry Several Paths; Preserve Every Projection.
                        let item = item(&self.inner, &self.held, event)?;
                        self.ready.push_back(item);
                    }
                    if let Some(item) = self.ready.pop_front() {
                        return Ok(Some(item));
                    }
                }
            })
            .await
    }

    fn stop(&mut self) {
        self.context.sync(|| self.context.span.record("outcome", "completed"));
        self.watcher.take();
        self.incoming.close();
        while self.incoming.try_recv().is_ok() {}
        self.ready.clear();
        self.initial = None;
        self.memory = None;
    }

    fn rescan(&mut self) -> ReadBatch<DirectoryEvent> {
        self.ready.clear();
        for _ in 0..self.inner.settings.filesystem.event_capacity.get() {
            if self.incoming.try_recv().is_err() {
                break;
            }
        }
        self.rescan.clone()
    }
}

impl Drop for DirectoryWatcher {
    fn drop(&mut self) {
        self.context.sync(|| drop(self.watcher.take()));
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn deliver<T>(outgoing: &mpsc::Sender<T>, value: T, overflow: &AtomicBool, changed: &Notify) {
    if outgoing.try_send(value).is_err() {
        signal(overflow, changed);
    }
}

fn signal(overflow: &AtomicBool, changed: &Notify) {
    overflow.store(true, Ordering::Release);
    changed.notify_one();
}

fn watch_error(error: notify::Error) -> Error {
    Error::backend("directory watch", crate::BackendFailure::Permanent, error)
}

fn selected_path(selection: &DirectorySelection, path: &Path) -> Result<Option<String>> {
    let Some(path) = relative(&selection.root, path, selection.recursive)? else { return Ok(None) };
    Ok(super::selection::selected(&path, &selection.rules).then_some(path))
}

fn entry(selection: &DirectorySelection, path: &Path) -> Result<Option<DirectoryEntry>> {
    let Some(relative) = selected_path(selection, path)? else { return Ok(None) };
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(crate::local::io_error(error)),
    };
    if crate::local::linked(&metadata) {
        return Ok(None);
    }
    crate::local::validate_local_path(path)?;
    Ok(Some(DirectoryEntry { path: relative, directory: metadata.is_dir(), length: if metadata.is_file() { metadata.len() } else { 0 } }))
}

fn events(selection: &DirectorySelection, event: Event) -> Result<Vec<DirectoryEvent>> {
    let root_changed = matches!(event.kind, EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_)))
        && event.paths.iter().any(|path| path == &selection.root);
    if event.need_rescan() || event.paths.is_empty() || root_changed {
        return Ok(vec![DirectoryEvent::RescanRequired]);
    }
    let mut result = Vec::new();
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Metadata(_) | ModifyKind::Any) => {
            for path in event.paths {
                if let Some(entry) = entry(selection, &path)? {
                    result.push(if matches!(event.kind, EventKind::Create(_)) {
                        DirectoryEvent::Created(entry)
                    } else {
                        DirectoryEvent::Modified(entry)
                    });
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                if let Some(path) = selected_path(selection, &path)? {
                    result.push(DirectoryEvent::Removed { path });
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 2 => {
            let from = selected_path(selection, &event.paths[0])?;
            let to = selected_path(selection, &event.paths[1])?;
            match (from, to) {
                (Some(from), Some(to)) => result.push(DirectoryEvent::Renamed { from, to }),
                (Some(path), None) => result.push(DirectoryEvent::Removed { path }),
                (None, Some(_)) => {
                    if let Some(entry) = entry(selection, &event.paths[1])? {
                        result.push(DirectoryEvent::Created(entry));
                    }
                }
                (None, None) => {}
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            for path in event.paths {
                if let Some(path) = selected_path(selection, &path)? {
                    result.push(DirectoryEvent::Removed { path });
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in event.paths {
                if let Some(entry) = entry(selection, &path)? {
                    result.push(DirectoryEvent::Created(entry));
                }
            }
        }
        EventKind::Access(_) => {}
        _ => result.push(DirectoryEvent::RescanRequired),
    }
    Ok(result)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn overflow_remains_observable() {
        let (outgoing, mut incoming) = mpsc::channel(1);
        let overflow = AtomicBool::new(false);
        let changed = Notify::new();
        deliver(&outgoing, 1, &overflow, &changed);
        deliver(&outgoing, 2, &overflow, &changed);
        assert!(overflow.swap(false, Ordering::AcqRel));
        changed.notified().await;
        assert_eq!(incoming.recv().await, Some(1));
        deliver(&outgoing, 3, &overflow, &changed);
        assert!(!overflow.load(Ordering::Acquire));
        assert_eq!(incoming.recv().await, Some(3));
    }

    #[test]
    fn rename_rescan_projection() {
        let root = std::path::PathBuf::from("root");
        let selection = DirectorySelection {
            root: root.clone(),
            recursive: true,
            rules: vec![harmos_domain::model::storage::DirectoryRule { pattern: "**".into(), include: true }],
        };
        let renamed =
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both))).add_path(root.join("before")).add_path(root.join("after"));
        assert_eq!(events(&selection, renamed).unwrap(), vec![DirectoryEvent::Renamed { from: "before".into(), to: "after".into() }]);
        assert_eq!(events(&selection, Event::new(EventKind::Other)).unwrap(), vec![DirectoryEvent::RescanRequired]);
    }
}
