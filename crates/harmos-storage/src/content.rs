/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Immutable Content Sources and Expiring Bounded Reads.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

// Third Party Imports

// Local Imports
use crate::execution::Execution;
use crate::{BackendFailure, ContentRange, Error, FileReference, Result, Snapshot, StorageFuture, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ContentLease {
    pub length: u64,
    pub sha256: [u8; 32],
    pub expires_at: SystemTime,
    pub max_bytes: NonZeroUsize,
    pub token: Vec<u8>,
}

#[derive(Clone)]
pub struct ContentRead {
    owner: Owner,
}

/* === Enums === */

#[derive(Clone)]
enum Owner {
    Stored { snapshot: Arc<Snapshot>, file: FileReference },
    Source { source: Arc<dyn ContentSource>, lease: ContentLease, clock: Execution, maximum: NonZeroUsize },
}

/* === Traits === */

pub trait ContentSource: Send + Sync + 'static {
    fn resolve<'a>(&'a self, reference: &'a str, scope: Option<&'a str>) -> StorageFuture<'a, ContentLease>;
    fn read<'a>(&'a self, lease: &'a ContentLease, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Vec<u8>>;
}

/* === Implementations === */

impl ContentRead {
    pub async fn open(
        source: Arc<dyn ContentSource>, reference: &str, scope: Option<&str>, clock: Execution, maximum: NonZeroUsize,
    ) -> Result<Self> {
        let lease = source.resolve(reference, scope).await.map_err(|error| provider_error("GetFileBytes.Resolve", error))?;
        if lease.token.len() > maximum.get() || lease.token.capacity() > maximum.get() {
            return Err(Error::ResourceExhausted { resource: "GetFileBytes lease token" });
        }
        if lease.expires_at <= clock.read().date {
            return Err(expired());
        }
        let reading = clock.read();
        let remaining = lease.expires_at.duration_since(reading.date).map_err(|_| expired())?;
        if reading.instant.checked_add(remaining).is_none() {
            return Err(Error::InvalidInput {
                field: "GetFileBytes",
                reason: "lease expiry exceeds clock capacity",
            });
        }
        Ok(Self {
            owner: Owner::Source { source, lease, clock, maximum },
        })
    }

    pub fn length(&self) -> u64 {
        match &self.owner {
            Owner::Stored { file, .. } => file.length,
            Owner::Source { lease, .. } => lease.length,
        }
    }

    pub fn sha256(&self) -> [u8; 32] {
        match &self.owner {
            Owner::Stored { file, .. } => file.sha256,
            Owner::Source { lease, .. } => lease.sha256,
        }
    }

    pub fn expires_at(&self) -> Option<SystemTime> {
        match &self.owner {
            Owner::Stored { .. } => None,
            Owner::Source { lease, .. } => Some(lease.expires_at),
        }
    }

    pub async fn get_bytes(&self, offset: u64, maximum: NonZeroUsize) -> Result<ContentRange> {
        let (source, lease, clock, bound) = match &self.owner {
            Owner::Stored { snapshot, file } => {
                let range = snapshot.get_file_bytes(file, offset, maximum).await?;
                return Ok(ContentRange {
                    data: range.data,
                    total_length: range.total_length,
                    sha256: range.sha256,
                });
            }
            Owner::Source { source, lease, clock, maximum } => (source, lease, clock, maximum),
        };
        if lease.expires_at <= clock.read().date {
            return Err(expired());
        }
        let maximum = maximum.min(*bound).min(lease.max_bytes);
        let range = crate::file_reads::read_content_range(lease.length, lease.sha256, offset, maximum, |offset, remaining| async move {
            let reading = clock.read();
            let remaining_time = lease.expires_at.duration_since(reading.date).map_err(|_| expired())?;
            if remaining_time.is_zero() {
                return Err(expired());
            }
            let deadline = reading.instant.checked_add(remaining_time).ok_or(Error::InvalidInput {
                field: "GetFileBytes",
                reason: "lease expiry exceeds clock capacity",
            })?;
            clock
                .timeout_at(deadline, source.read(lease, offset, remaining))
                .await
                .map_err(|_| expired())?
                .map_err(|error| provider_error("GetFileBytes.Read", error))
        })
        .await?;
        if lease.expires_at <= clock.read().date {
            return Err(expired());
        }
        Ok(range)
    }
}

impl StorageManager {
    /// Register One Application Owner; Its Sources Remain Owned by This Manager's Lifetime.
    pub fn register_content_source(&self, source: Arc<dyn ContentSource>) -> Result<()> {
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "Register Content Source" });
        }
        let mut current = self.inner.content.lock().unwrap();
        if current.is_some() {
            return Err(Error::AlreadyRegistered {
                domain: "Register Content Source",
                name: "application content".into(),
            });
        }
        *current = Some(source);
        Ok(())
    }

    /// Open One Immutable Read Without Copying Application-Owned Bytes or Metadata.
    pub async fn open_content(&self, file: &FileReference, scope: Option<&str>, at: Option<&crate::StorageCursor>) -> Result<ContentRead> {
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "GetFileBytes" });
        }
        if &file.position.history == self.inner.primary.get_history() {
            let saved = self.get_saved_position();
            let snapshot = self.snapshot(at.unwrap_or(&saved)).await?;
            snapshot.check_file_reference(file, "GetFileBytes").await?;
            return Ok(ContentRead {
                owner: Owner::Stored {
                    snapshot: Arc::new(snapshot),
                    file: file.clone(),
                },
            });
        }
        if at.is_some() {
            return Err(Error::InvalidInput {
                field: "GetFileBytes",
                reason: "application content requires its issued read lease",
            });
        }
        let source = self.inner.content.lock().unwrap().clone().ok_or(Error::NotReady {
            domain: "GetFileBytes application content source",
        })?;
        let read = self
            .inner
            .bounded(
                "GetFileBytes.Resolve",
                Box::pin(ContentRead::open(
                    source,
                    file.metadata.id.as_str(),
                    scope,
                    self.inner.execution.clone(),
                    self.inner.settings.transfer_chunk_bytes(),
                )),
            )
            .await?;
        if read.length() != file.length || read.sha256() != file.sha256 {
            return Err(Error::InvalidInput {
                field: "GetFileBytes",
                reason: "application content reference does not match source metadata",
            });
        }
        Ok(read)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn expired() -> Error {
    Error::InvalidInput {
        field: "GetFileBytes",
        reason: "content lease expired; restart the read",
    }
}

fn provider_error(action: &'static str, error: Error) -> Error {
    Error::backend(action, error.backend_failure().unwrap_or(BackendFailure::Permanent), error)
}

/* === Tests === */
