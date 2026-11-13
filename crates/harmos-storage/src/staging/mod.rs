/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(super) mod checkpoint;
mod creation;
mod mutations;
mod perform;
mod producer;
mod session;
mod state;

/* === Re-Exports === */

pub(crate) use perform::perform;

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::manager::{Operation, Pending, State, Target, encoded_size};
use super::*;
use crate::{Error, Result};
use session::Session;

/* === Constants === */

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct FileProductionStatus {
    pub progress: FileProgress,
    pub writer_generation: Option<u64>,
    pub accepted_length: u64,
    pub durable_length: u64,
}

pub struct StorageFile {
    manager: StorageManager,
    id: u64,
    progress: Arc<Mutex<Progress>>,
    session: Option<FileSession>,
    append: Option<(u64, crate::FileChunk, Option<u64>)>,
}

struct CancelProduction {
    manager: StorageManager,
    id: u64,
    armed: bool,
    generation: Option<u64>,
}

#[derive(Default)]
struct Progress {
    generation: u64,
    ready: bool,
    closed: bool,
    completed: u64,
    length: u64,
    reference: Option<FileReference>,
    failure: Option<Arc<Error>>,
}

pub(super) struct FileState {
    pub(super) context: crate::execution::Context,
    pub(super) queued: Option<tracing::Span>,
    metadata: Arc<FileMetadata>,
    staging: StagedFile,
    session: Option<Session>,
    mutation: Option<mutations::Mutation>,
    extra: Option<StagedFile>,
    progress: Arc<Mutex<Progress>>,
    hash: Sha256,
    length: u64,
    durable_length: u64,
    ticket: u64,
    command: Option<Command>,
    abandoned: bool,
    in_flight: bool,
    retrying: bool,

    reference_bytes: usize,
}

/* === Enums === */

enum Command {
    Create,
    Resume { expected: u64, next: u64 },
    Write(crate::FileChunk),
    Replay { offset: u64, bytes: crate::FileChunk },
    Finish { target: Option<Target>, sha256: [u8; 32] },
    Abort,
}

enum Await {
    Ready,
    Written(u64),
    Finished,
    Aborted,
}

pub(super) enum FileAction {
    Create { id: u64, metadata: Arc<FileMetadata>, staging: StagedFile, resumable: bool },
    Resume { id: u64, staging: StagedFile, expected: u64, next: u64 },
    Write { id: u64, staging: StagedFile, offset: u64, bytes: crate::FileChunk },
    Replay { id: u64, staging: StagedFile, offset: u64, bytes: crate::FileChunk },
    Finish { id: u64, staging: StagedFile, metadata: Arc<FileMetadata>, target: Target, length: u64, sha256: [u8; 32] },
    Abandon { id: u64, staging: StagedFile, extra: Option<StagedFile> },
    Abort { id: u64, staging: StagedFile, extra: Option<StagedFile> },
    Refuse { id: u64 },
}

pub(super) enum FileCompleted {
    Created,
    Resumed(u64),
    Written,
    Replayed(bool),
    Finished(Box<FileReference>),
    Aborted,
}

/* === Traits === */

/* === Implementations === */

impl FileAction {
    pub(crate) fn independent(&self) -> bool {
        !matches!(self, Self::Finish { .. } | Self::Refuse { .. })
    }
}

impl StorageManager {
    /// Status Is Read From the Production Owner, Without Creating a Writer.
    pub async fn get_file_production_status(&self, file: &StagedFile) -> Result<FileProductionStatus> {
        self.inner.primary.get_history().check(&file.history)?;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing || !state.ready {
                return Err(closed());
            }
            if let Some(active) = state.files.values().find(|active| active.staging == *file) {
                let progress = active.production(file).expect("selected production")?;
                let generation = active.session.as_ref().map(|session| session.generation);
                let pending = match &active.command {
                    Some(Command::Write(bytes)) => bytes.len() as u64,
                    _ => 0,
                };
                return Ok(FileProductionStatus {
                    progress,
                    writer_generation: generation,
                    accepted_length: progress.length + pending,
                    durable_length: active.durable_length,
                });
            }
        }
        let progress = self.get_file_production(file).await?;
        Ok(FileProductionStatus {
            progress,
            writer_generation: None,
            accepted_length: progress.length,
            durable_length: if progress.phase == FilePhase::Finished { progress.length } else { 0 },
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn file_bytes(metadata: &FileMetadata, history: &StoreHistory) -> Result<usize> {
    let reference = FileReference {
        position: StorageCursor {
            history: history.clone(),
            sequence: u64::MAX,
        },
        metadata: metadata.clone(),
        length: u64::MAX,
        sha256: [255; 32],
    };
    encoded_size(&reference)
}

fn closed() -> Error {
    Error::Closing { domain: "storage file" }
}

fn size_error() -> Error {
    Error::InvalidInput {
        field: "file admission",
        reason: "size or position overflow",
    }
}

fn admission(state: &State) -> Result<()> {
    if state.status.closing {
        return Err(closed());
    }
    if !state.ready || !state.status.running {
        return Err(Error::NotReady { domain: "storage" });
    }
    if let Some(source) = &state.status.failure {
        return Err(Error::Persistence { source: source.clone() });
    }
    Ok(())
}

/* === Tests === */
