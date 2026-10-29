/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Capture An Opened Source Into One Retained Producer.

use crate::{ByteRange, CaptureConsistency, FileChunk, FileReader, FileReference, Result, StorageFile, StorageManager};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

type Pending = Pin<Box<dyn Future<Output = (State, Result<Step>)> + Send>>;

/// Retains the original opened source, producer and exact in-flight copy across cancellation.
pub struct OpenedCapture {
    state: Option<State>,
    pending: Mutex<Option<Pending>>,
    acknowledged: Option<Arc<AtomicU64>>,
    initial_offset: u64,
    request: Option<Request>,
}

struct State {
    session: crate::StorageManager<crate::ErasedCatalog>,
    file: std::fs::File,
    range: ByteRange,
    reader: Option<FileReader>,
    target: StorageFile,
    consistency: CaptureConsistency,
    chunk: Option<FileChunk>,
    offset: u64,
    acknowledged: Option<Arc<AtomicU64>>,
    prefix: Option<crate::ProductionFollower>,
    remaining_prefix: u64,
    matched: usize,
    conflict: bool,
    request: Request,
    action: Action,
    in_round: bool,
    observed: Option<(u64, std::time::SystemTime)>,
    sealed: bool,
    finished: Option<FileReference>,
    frozen: Option<crate::FrozenFile>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Request {
    Finish,
    Synchronize(u64),
    FinishAt(u64),
    Interrupt,
}

#[derive(Clone, Copy)]
enum Action {
    Recover,
    Open,
    Prefix,
    Read,
    Match,
    Check,
    Write,
    Seal,
}

enum Step {
    Continue,
    Synchronized(bool),
    Finished,
}

impl<C> StorageManager<C> {
    pub fn capture_frozen_file(&self, source: crate::FrozenFile, target: StorageFile) -> Result<OpenedCapture> {
        let file = source.opened_file()?;
        let range = ByteRange { offset: 0, limit: Some(source.length()) };
        let mut capture = self.capture_opened_file(file, target, range, CaptureConsistency::Stable);
        let state = capture.state.as_mut().expect("new capture retains source state");
        if Arc::ptr_eq(&self.inner, &source.session.inner) {
            state.session = source.session.clone();
        }
        state.frozen = Some(source);
        Ok(capture)
    }

    pub fn capture_opened_file(
        &self, file: std::fs::File, target: StorageFile, range: ByteRange, consistency: CaptureConsistency,
    ) -> OpenedCapture {
        let offset = target.length();
        OpenedCapture {
            state: Some(State {
                session: self.erased(),
                file,
                range,
                reader: None,
                target,
                consistency,
                chunk: None,
                offset,
                acknowledged: None,
                prefix: None,
                remaining_prefix: offset,
                matched: 0,
                conflict: false,
                request: Request::Finish,
                action: Action::Recover,
                in_round: false,
                observed: None,
                sealed: false,
                finished: None,
                frozen: None,
            }),
            pending: Mutex::new(None),
            acknowledged: None,
            initial_offset: offset,
            request: None,
        }
    }
}

impl OpenedCapture {
    pub fn acknowledged(&self) -> u64 {
        self.acknowledged.as_ref().map_or(self.initial_offset, |value| value.load(Ordering::Acquire))
    }

    pub async fn finish(&mut self) -> Result<FileReference> {
        match self.run(Request::Finish).await? {
            Step::Finished => Ok(self.finished()),
            _ => unreachable!("terminal capture returns its reference"),
        }
    }

    /// Admits the selected visible prefix without sealing the original producer.
    /// False discards an unstable, unaccepted observation; accepted bytes remain retained.
    pub async fn synchronize(&mut self, visible_length: u64) -> Result<bool> {
        match self.run(Request::Synchronize(visible_length)).await? {
            Step::Synchronized(stable) => Ok(stable),
            _ => unreachable!("nonterminal capture returns its observation"),
        }
    }

    /// Verifies and seals one quiescent, range-relative visible prefix.
    pub async fn finish_at(&mut self, visible_length: u64) -> Result<FileReference> {
        match self.run(Request::FinishAt(visible_length)).await? {
            Step::Finished => Ok(self.finished()),
            _ => unreachable!("terminal capture returns its reference"),
        }
    }

    /// Settles an accepted append and seals that prefix without another source read.
    pub async fn interrupt(&mut self) -> Result<FileReference> {
        match self.run(Request::Interrupt).await? {
            Step::Finished => Ok(self.finished()),
            _ => unreachable!("interrupted capture returns its reference"),
        }
    }

    fn finished(&mut self) -> FileReference {
        self.state.as_mut().expect("capture state retained").finished.take().expect("finished capture retains its reference")
    }

    async fn run(&mut self, request: Request) -> Result<Step> {
        let changed = self.request != Some(request);
        if changed && self.pending.get_mut().expect("capture pending owner is healthy").is_some() {
            // Resume Only The Already Accepted Step. Interruption Must Not Start
            // Another Source Read Or Append From The Cancelled Observation.
            let settled = self.step().await;
            if request != Request::Interrupt {
                settled?;
            }
        }
        if self.state.as_ref().is_some_and(|state| changed || !state.in_round) {
            self.state.as_mut().expect("capture state retained").reset(request)?;
            self.request = Some(request);
        }
        loop {
            match self.step().await? {
                Step::Continue => {}
                outcome => return Ok(outcome),
            }
        }
    }

    async fn step(&mut self) -> Result<Step> {
        let pending = self.pending.get_mut().expect("capture pending owner is healthy");
        if pending.is_none() {
            if self.acknowledged.is_none() {
                let acknowledged = Arc::new(AtomicU64::new(self.initial_offset));
                self.state.as_mut().expect("capture owns its state").acknowledged = Some(acknowledged.clone());
                self.acknowledged = Some(acknowledged);
            }
            let state = self.state.take().expect("capture owns its state");
            *pending = Some(Box::pin(work(state)));
        }
        let (state, result) = pending.as_mut().expect("accepted capture retained").await;
        *pending = None;
        self.state = Some(state);
        result
    }
}

impl State {
    fn reset(&mut self, request: Request) -> Result<()> {
        self.finished = None;
        if self.sealed && matches!(request, Request::Synchronize(_)) {
            return Err(crate::Error::InvalidInput {
                field: "opened capture",
                reason: "producer is already sealed",
            });
        }
        if self.sealed && matches!(request, Request::FinishAt(length) if length != self.offset) {
            return Err(conflict());
        }
        self.request = request;
        self.in_round = true;
        self.action = if self.sealed { Action::Seal } else { Action::Recover };
        self.reader = None;
        self.prefix = None;
        self.chunk = None;
        self.matched = 0;
        self.remaining_prefix = 0;
        Ok(())
    }

    async fn advance(&mut self) -> Result<Step> {
        if self.conflict && self.request != Request::Interrupt {
            return Err(conflict());
        }
        match self.action {
            Action::Recover => {
                self.target.retry_append().await?;
                self.offset = self.target.length();
                self.acknowledged.as_ref().expect("acknowledgement retained").store(self.offset, Ordering::Release);
                self.action = if self.request == Request::Interrupt { Action::Seal } else { Action::Open };
            }
            Action::Open => {
                let mut range = self.range;
                if let Request::Synchronize(length) | Request::FinishAt(length) = self.request {
                    if length < self.offset {
                        self.conflict = true;
                        return Err(conflict());
                    }
                    if range.limit.is_some_and(|limit| length > limit) {
                        return Err(crate::Error::InvalidInput {
                            field: "opened capture",
                            reason: "visible length exceeds selected range",
                        });
                    }
                    range.limit = Some(length);
                }
                let file = self.file.try_clone().map_err(|error| {
                    let path = crate::local::opened_path(&self.file);
                    crate::local::io_error("clone capture source", &path, error)
                })?;
                let frozen = self.frozen.as_ref().map(|source| Arc::clone(&source.backing));
                let mut reader = self.session.open_file_handle(file, range, frozen).await?;
                self.remaining_prefix = self.offset;
                if let Request::Synchronize(length) | Request::FinishAt(length) = self.request {
                    let observation = reader.observation().expect("opened capture reads its original handle");
                    let retained_end = self.range.offset.checked_add(self.offset).ok_or_else(conflict)?;
                    if observation.0 < retained_end {
                        self.conflict = true;
                        return Err(conflict());
                    }
                    let visible_end = self.range.offset.checked_add(length).ok_or_else(conflict)?;
                    if observation.0 < visible_end {
                        return Err(crate::Error::InvalidInput {
                            field: "opened capture",
                            reason: "visible range exceeds opened source",
                        });
                    }
                    if matches!(self.request, Request::Synchronize(_)) && self.observed == Some(observation) {
                        reader.start_opened(retained_end)?;
                        self.remaining_prefix = 0;
                    }
                }
                self.reader = Some(reader);
                self.action = if self.remaining_prefix == 0 { Action::Read } else { Action::Prefix };
            }
            Action::Prefix => {
                let token = self.target.session().ok_or(crate::Error::InvalidInput {
                    field: "opened capture",
                    reason: "existing producer is not resumable",
                })?;
                let staging = crate::StagedFile {
                    history: token.history.clone(),
                    id: token.id.clone(),
                };
                self.prefix = Some(self.session.follow_production(&staging, 0).await?);
                self.action = Action::Read;
            }
            Action::Read => {
                let Some(batch) = self.reader.as_mut().expect("opened source retained").next_raw(false).await? else {
                    if self.remaining_prefix != 0 {
                        self.conflict = true;
                        return Err(conflict());
                    }
                    self.reader.as_ref().expect("opened source retained").completion()?;
                    self.action = Action::Check;
                    return Ok(Step::Continue);
                };
                self.chunk = Some(FileChunk::new(batch.bytes));
                self.action = if self.remaining_prefix == 0 { Action::Check } else { Action::Match };
            }
            Action::Match => {
                let chunk = self.chunk.as_ref().expect("exact capture bytes retained");
                let count = (chunk.len() - self.matched).min(usize::try_from(self.remaining_prefix).unwrap_or(usize::MAX));
                let batch = self
                    .prefix
                    .as_mut()
                    .expect("retained prefix reader")
                    .next_batch(std::num::NonZeroUsize::new(count).unwrap())
                    .await?
                    .ok_or_else(conflict)?;
                let count = batch.bytes().len();
                if batch.bytes() != &chunk.as_slice()[self.matched..self.matched + count] {
                    self.conflict = true;
                    return Err(conflict());
                }
                self.matched += count;
                self.remaining_prefix -= count as u64;
                if self.matched == chunk.len() {
                    self.chunk = None;
                    self.matched = 0;
                    self.action = Action::Read;
                } else if self.remaining_prefix == 0 {
                    self.action = Action::Check;
                }
                if self.remaining_prefix == 0 {
                    self.prefix = None;
                }
            }
            Action::Check => {
                let reader = self.reader.as_ref().expect("opened source retained");
                if matches!(self.request, Request::Synchronize(_) | Request::FinishAt(_)) {
                    if !reader.observation_unchanged().await? {
                        self.reader = None;
                        self.prefix = None;
                        self.chunk = None;
                        self.matched = 0;
                        self.in_round = false;
                        if matches!(self.request, Request::FinishAt(_)) {
                            return Err(crate::Error::InvalidInput {
                                field: "opened capture",
                                reason: "source changed during final capture",
                            });
                        }
                        return Ok(Step::Synchronized(false));
                    }
                    if self.chunk.is_none() {
                        self.observed = reader.observation();
                        if matches!(self.request, Request::Synchronize(_)) {
                            self.in_round = false;
                            self.reader = None;
                            return Ok(Step::Synchronized(true));
                        }
                    }
                } else if self.chunk.is_none() && self.consistency == CaptureConsistency::Stable {
                    reader.check_unchanged().await?;
                }
                self.action = if self.chunk.is_some() { Action::Write } else { Action::Seal };
            }
            Action::Write => {
                let chunk = self.chunk.as_mut().expect("exact capture bytes retained");
                let suffix = chunk.slice(self.matched..chunk.len());
                self.target.write_retained_at(self.offset, suffix.clone()).await?;
                self.offset += suffix.len() as u64;
                self.acknowledged.as_ref().expect("acknowledgement retained").store(self.offset, Ordering::Release);
                self.chunk = None;
                self.matched = 0;
                self.action = Action::Read;
            }
            Action::Seal => {
                let result = self.target.finish().await;
                if result.is_err() && self.reader.is_some() && self.request != Request::Interrupt {
                    // A Returned Failure Starts A Retry At Source Validation;
                    // Cancellation Still Resumes The Exact Accepted Seal Step.
                    self.action = Action::Check;
                }
                self.finished = Some(result?);
                self.sealed = true;
                self.action = if self.request == Request::Finish && self.reader.is_some() { Action::Check } else { Action::Seal };
                return Ok(Step::Finished);
            }
        }
        Ok(Step::Continue)
    }
}

async fn work(mut state: State) -> (State, Result<Step>) {
    let result = state.advance().await;
    (state, result)
}

fn conflict() -> crate::Error {
    crate::Error::InvalidInput {
        field: "opened capture",
        reason: "source differs from retained producer prefix",
    }
}
