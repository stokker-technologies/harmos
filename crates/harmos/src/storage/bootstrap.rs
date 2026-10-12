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
use std::io::{self, Write};
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
// Local Imports
use super::manager::{ErasedCatalog, encoded_size};
use super::replication::Replica;
use super::selection::Entry;
use super::{
    BackendSnapshot, FileReference, ObjectEntry, QueryLimits, ReadHistory, ReadKind, ReadLimits,
    ReadSelection, ReadView, StorageCursor, StorageEntry, StorageManager, StoragePart,
    StorageSchema, StoredRecord, StoredRow,
};
use crate::Identifier;
use crate::supervision::executor;
use crate::{BackendFailure, Error, Result};

/* === Constants === */
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedCoverage {
    pub cut: StorageCursor,
    pub complete_after: u64,
    pub retention_generation: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedSnapshot {
    pub snapshot: BackendSnapshot,
    pub coverage: RetainedCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DecodedBootstrapId")]
pub struct BootstrapId {
    cut: StorageCursor,
    complete_after: u64,
    retention_generation: NonZeroU64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedBootstrapId {
    cut: StorageCursor,
    complete_after: u64,
    retention_generation: NonZeroU64,
}

#[derive(Clone, Copy)]
pub struct RetainedBatch<'a> {
    identity: &'a BootstrapId,
    part: StoragePart<'a>,
    first: &'a StorageCursor,
    last: &'a StorageCursor,
    len: usize,
    bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionDigest {
    entries: u64,
    encoded_bytes: u64,
    last_position: u64,
    sha256: [u8; 32],
    complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapManifest {
    identity: BootstrapId,
    collections: [CollectionDigest; 5],
}

struct HashWriter(Sha256);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl RetainedCoverage {
    pub fn check(&self) -> Result<()> {
        if self.complete_after > self.cut.sequence {
            return Err(invalid("complete-history floor exceeds the retained cut"));
        }
        Ok(())
    }

    pub fn requires_bootstrap(&self, saved: &StorageCursor) -> Result<bool> {
        self.check()?;
        if !self.cut.covers(saved)? {
            return Err(invalid("target is ahead of the retained source cut"));
        }
        Ok(saved.sequence < self.complete_after)
    }
}

impl RetainedSnapshot {
    pub fn check(&self) -> Result<()> {
        self.coverage.check()?;
        if self.snapshot.saved != self.coverage.cut || self.snapshot.token.is_empty() {
            return Err(invalid("snapshot pin and retained coverage differ"));
        }
        Ok(())
    }
}

impl TryFrom<DecodedBootstrapId> for BootstrapId {
    type Error = Error;

    fn try_from(value: DecodedBootstrapId) -> Result<Self> {
        if value.complete_after > value.cut.sequence {
            return Err(invalid("complete-history floor exceeds the retained cut"));
        }
        Ok(Self {
            cut: value.cut,
            complete_after: value.complete_after,
            retention_generation: value.retention_generation,
        })
    }
}

impl BootstrapId {
    pub fn new(coverage: &RetainedCoverage) -> Result<Self> {
        coverage.check()?;
        Ok(Self {
            cut: coverage.cut.clone(),
            complete_after: coverage.complete_after,
            retention_generation: coverage.retention_generation,
        })
    }

    pub fn cut(&self) -> &StorageCursor {
        &self.cut
    }

    pub fn complete_after(&self) -> u64 {
        self.complete_after
    }

    pub fn retention_generation(&self) -> NonZeroU64 {
        self.retention_generation
    }
}

impl<'a> RetainedBatch<'a> {
    pub fn new(
        identity: &'a BootstrapId,
        part: StoragePart<'a>,
        limits: ReadLimits,
    ) -> Result<Self> {
        let mut entries = part.entries();
        let first = entries.next().ok_or(invalid("retained batch is empty"))?;
        let mut last = first.position();
        if last.sequence == 0 || !identity.cut.covers(last)? {
            return Err(invalid("retained entry is outside the import cut"));
        }
        let mut bytes = first.encoded_bytes()?;
        let mut len = 1usize;
        limits.check(len, bytes)?;
        for entry in entries {
            let position = entry.position();
            if position.sequence <= last.sequence || !identity.cut.covers(position)? {
                return Err(invalid(
                    "retained positions are repeated, unordered or outside the cut",
                ));
            }
            bytes = bytes
                .checked_add(entry.encoded_bytes()?)
                .ok_or(invalid("retained byte count overflow"))?;
            len = len
                .checked_add(1)
                .ok_or(invalid("retained entry count overflow"))?;
            limits.check(len, bytes)?;
            last = position;
        }
        Ok(Self {
            identity,
            part,
            first: first.position(),
            last,
            len,
            bytes,
        })
    }

    pub fn identity(&self) -> &BootstrapId {
        self.identity
    }

    pub fn part(&self) -> StoragePart<'a> {
        self.part
    }

    pub fn first_position(&self) -> &StorageCursor {
        self.first
    }

    pub fn last_position(&self) -> &StorageCursor {
        self.last
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn encoded_bytes(&self) -> usize {
        self.bytes
    }
}

impl CollectionDigest {
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn entries(&self) -> u64 {
        self.entries
    }

    pub fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    pub fn last_position(&self) -> u64 {
        self.last_position
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl BootstrapManifest {
    pub fn new(identity: BootstrapId) -> Self {
        Self {
            identity,
            collections: std::array::from_fn(|_| CollectionDigest {
                entries: 0,
                encoded_bytes: 0,
                last_position: 0,
                sha256: Sha256::digest([]).into(),
                complete: false,
            }),
        }
    }

    pub fn identity(&self) -> &BootstrapId {
        &self.identity
    }

    pub fn collection(&self, kind: ReadKind) -> &CollectionDigest {
        &self.collections[index(kind)]
    }

    pub fn finish_collection(&mut self, kind: ReadKind) {
        self.collections[index(kind)].complete = true;
    }

    pub fn check_complete(&self) -> Result<()> {
        if self
            .collections
            .iter()
            .any(|collection| !collection.complete)
        {
            return Err(invalid("retained export has unfinished collections"));
        }
        Ok(())
    }

    pub fn record(&mut self, batch: RetainedBatch<'_>) -> Result<()> {
        if batch.identity != &self.identity {
            return Err(invalid("batch belongs to another import identity"));
        }
        let kind = match batch.part {
            StoragePart::Schemas(_) => ReadKind::Schemas,
            StoragePart::Objects(_) => ReadKind::Objects,
            StoragePart::Records(_) => ReadKind::Records,
            StoragePart::Stream(_) => ReadKind::Stream,
            StoragePart::Files(_) => ReadKind::Files,
        };
        let collection = &self.collections[index(kind)];
        if collection.complete || batch.first.sequence <= collection.last_position {
            return Err(invalid("manifest progress is repeated or unordered"));
        }
        let mut updated = collection.clone();
        updated.entries = updated
            .entries
            .checked_add(u64::try_from(batch.len).map_err(|_| invalid("manifest count overflow"))?)
            .ok_or(invalid("manifest count overflow"))?;
        updated.encoded_bytes = updated
            .encoded_bytes
            .checked_add(
                u64::try_from(batch.bytes).map_err(|_| invalid("manifest bytes overflow"))?,
            )
            .ok_or(invalid("manifest bytes overflow"))?;
        for entry in batch.part.entries() {
            let mut writer = HashWriter(Sha256::new());
            let result = match entry {
                StorageEntry::Schema(value) => serde_json::to_writer(&mut writer, value),
                StorageEntry::Object(value) => serde_json::to_writer(&mut writer, value),
                StorageEntry::Record(value) => serde_json::to_writer(&mut writer, value),
                StorageEntry::Row(value) => serde_json::to_writer(&mut writer, value),
                StorageEntry::File(value) => serde_json::to_writer(&mut writer, value),
            };
            result.map_err(|error| {
                Error::backend("bootstrap manifest", BackendFailure::Corrupt, error)
            })?;
            let mut digest = Sha256::new();
            digest.update(updated.sha256);
            digest.update(writer.0.finalize());
            updated.sha256 = digest.finalize().into();
        }
        updated.last_position = batch.last.sequence;
        self.collections[index(kind)] = updated;
        Ok(())
    }
}

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */
pub(super) async fn copy(
    manager: &StorageManager<ErasedCatalog>,
    replica: &Replica,
    through: &StorageCursor,
) -> Result<()> {
    replica.check_copying()?;
    let memory_bytes = manager
        .inner
        .settings
        .replication_limits
        .bytes
        .get()
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(manager.inner.schema_encoding.decoding_overhead()))
        .and_then(NonZeroUsize::new)
        .ok_or(invalid("bootstrap execution bound overflow"))?;
    let execution = QueryLimits {
        memory_bytes,
        spill_bytes: 0,
        timeout: manager.inner.settings.io_timeout,
    };
    let retained = manager
        .inner
        .bounded(manager.inner.primary.read_retained_view(through, execution))
        .await?;
    let coverage = retained.coverage;
    replica.state.lock().unwrap().snapshot = Some(retained.snapshot);
    coverage.check()?;
    let snapshot = {
        let state = replica.state.lock().unwrap();
        let snapshot = state.snapshot.as_ref().unwrap();
        if coverage.cut != *through
            || snapshot.saved != coverage.cut
            || snapshot.token.is_empty()
            || snapshot.token.len() > manager.inner.settings.snapshot_bytes.get()
        {
            return Err(invalid(
                "retained snapshot differs from requested cut or token bound",
            ));
        }
        snapshot.clone()
    };
    let saved = manager
        .inner
        .bounded(replica.config.backend.cursor_saved())
        .await?;
    if !coverage.requires_bootstrap(&saved)? {
        return Err(invalid("source gap is not certified by retained coverage"));
    }
    let identity = BootstrapId::new(&coverage)?;
    // Install Cleanup Ownership Before Fallible Creation
    replica.state.lock().unwrap().bootstrap = Some(identity.clone());
    manager
        .inner
        .bounded(
            replica
                .config
                .backend
                .begin_bootstrap(&identity, &saved, execution),
        )
        .await?;
    let sequence = NEXT_VIEW
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| invalid("bootstrap view identity overflow"))?;
    let id = Identifier::new(format!("bootstrap-view-{sequence}")).unwrap();
    let view = ReadView::new(id, through.clone(), through.clone(), snapshot)?
        .with_execution(Some(execution));
    let mut manifest = BootstrapManifest::new(identity.clone());
    export::<StorageSchema>(manager, replica, &view, &identity, &mut manifest).await?;
    export::<ObjectEntry>(manager, replica, &view, &identity, &mut manifest).await?;
    export::<StoredRecord>(manager, replica, &view, &identity, &mut manifest).await?;
    export::<StoredRow>(manager, replica, &view, &identity, &mut manifest).await?;
    export::<FileReference>(manager, replica, &view, &identity, &mut manifest).await?;
    manifest.check_complete()?;
    replica.check_copying()?;
    let receipt = manager
        .inner
        .bounded(replica.config.backend.finish_bootstrap(&manifest))
        .await?;
    if receipt != *through {
        return Err(invalid("target committed a different bootstrap cut"));
    }
    replica.state.lock().unwrap().status.saved = Some(receipt);
    manager.inner.notify();
    Ok(())
}

/* === Private Functions === */
async fn export<T: Entry>(
    manager: &StorageManager<ErasedCatalog>,
    replica: &Replica,
    view: &ReadView,
    identity: &BootstrapId,
    manifest: &mut BootstrapManifest,
) -> Result<()> {
    let mut selection = ReadSelection::new(T::KIND);
    if matches!(T::KIND, ReadKind::Objects | ReadKind::Files) {
        selection.history = ReadHistory::Retained;
    }
    let limits = T::read_limits(
        manager.inner.settings.replication_limits,
        manager.inner.schema_encoding,
    );
    let batch_limits = ReadLimits {
        bytes: limits.bytes.min(manager.inner.settings.max_batch_bytes),
        ..limits
    };
    let mut cursor = None;
    loop {
        executor::yield_now().await;
        replica.check_copying()?;
        let page = manager
            .inner
            .bounded(T::fetch(
                manager.inner.primary.as_ref(),
                view,
                &selection,
                cursor.as_ref(),
                limits,
            ))
            .await?;
        page.check_positions(view, &selection, cursor.as_ref(), T::position)?;
        let mut bytes = 0usize;
        for entry in &page.entries {
            if !entry.matches(&selection) {
                return Err(invalid("retained page returned a different selection"));
            }
            entry.validate(&manager.inner, limits)?;
            bytes = bytes
                .checked_add(encoded_size(entry)?)
                .ok_or(invalid("retained page size overflow"))?;
        }
        limits.check(page.entries.len(), bytes)?;
        let mut start = 0;
        while start < page.entries.len() {
            replica.check_copying()?;
            let mut end = start;
            let mut bytes = 0usize;
            while end < page.entries.len() && end - start < batch_limits.rows.get() {
                let size = encoded_size(&page.entries[end])?;
                if size > batch_limits.bytes.get() - bytes {
                    break;
                }
                bytes += size;
                end += 1;
            }
            if start == end {
                return Err(invalid(
                    "one retained entry exceeds the physical batch bound",
                ));
            }
            let part = T::part(&page.entries[start..end]);
            if let StoragePart::Files(files) = part {
                for file in files {
                    super::transfer::file(manager, replica, view, file, Some(identity)).await?;
                }
            }
            let batch = RetainedBatch::new(identity, part, batch_limits)?;
            manager
                .inner
                .bounded(replica.config.backend.write_bootstrap(batch))
                .await?;
            manifest.record(batch)?;
            start = end;
        }
        cursor = page.next;
        if cursor.is_none() {
            manifest.finish_collection(T::KIND);
            return Ok(());
        }
    }
}

fn index(kind: ReadKind) -> usize {
    match kind {
        ReadKind::Schemas => 0,
        ReadKind::Objects => 1,
        ReadKind::Records => 2,
        ReadKind::Stream => 3,
        ReadKind::Files => 4,
    }
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "replica bootstrap",
        reason,
    }
}

/* === Tests === */
