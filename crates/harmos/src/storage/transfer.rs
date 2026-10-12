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
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::manager::encoded_size;
use super::replication::Replica;
use super::selection::Entry;
use super::*;
use crate::supervision::executor;
use crate::{Error, Identifier, Result};

/* === Constants === */
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Cursor<T> {
    entries: VecDeque<T>,
    next: Option<ReadCursor>,
    done: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T: Entry> Cursor<T> {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            next: None,
            done: false,
        }
    }

    fn first(&self) -> Option<u64> {
        self.entries.front().map(|entry| entry.position().sequence)
    }

    fn batch(&mut self, before: Option<u64>, after: u64) -> Result<Vec<T>> {
        let mut previous = after;
        let mut entries = Vec::new();
        while self
            .entries
            .front()
            .is_some_and(|entry| before.is_none_or(|before| entry.position().sequence < before))
        {
            let entry = self.entries.pop_front().unwrap();
            if previous.checked_add(1) != Some(entry.position().sequence) {
                return Err(invalid("source has a missing capture position"));
            }
            previous = entry.position().sequence;
            entries.push(entry);
        }
        Ok(entries)
    }

    async fn fill(
        &mut self,
        manager: &StorageManager<crate::storage::ErasedCatalog>,
        replica: &Replica,
        view: &ReadView,
        after: u64,
    ) -> Result<()> {
        if !self.entries.is_empty() || self.done {
            return Ok(());
        }
        replica.check_copying()?;
        let mut selection = ReadSelection::new(T::KIND);

        selection.after = after;
        let limits = T::read_limits(
            manager.inner.settings.replication_limits,
            manager.inner.schema_encoding,
        );
        let page = manager
            .inner
            .bounded(T::fetch(
                manager.inner.primary.as_ref(),
                view,
                &selection,
                self.next.as_ref(),
                limits,
            ))
            .await?;
        page.check_positions(view, &selection, self.next.as_ref(), T::position)?;
        let bytes = page.entries.iter().try_fold(0usize, |sum, entry| {
            if !entry.matches(&selection) {
                return Err(invalid("source returned a different selection"));
            }
            entry.validate(&manager.inner, limits)?;
            sum.checked_add(encoded_size(entry)?)
                .ok_or(invalid("source page size overflow"))
        })?;
        limits.check(page.entries.len(), bytes)?;
        self.done = page.next.is_none();
        self.next = page.next;
        self.entries = page.entries.into();
        Ok(())
    }
}

/* === Public Functions === */
pub(super) async fn copy(
    manager: &StorageManager<crate::storage::ErasedCatalog>,
    replica: &Replica,
    after: &StorageCursor,
    through: &StorageCursor,
) -> Result<()> {
    replica.check_copying()?;
    let identity = NEXT_VIEW
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| invalid("replica view identity overflow"))?;
    let id = Identifier::new(format!("replica-view-{identity}")).unwrap();
    let snapshot = manager
        .inner
        .bounded(manager.inner.primary.read_view(through))
        .await?;
    replica.state.lock().unwrap().snapshot = Some(snapshot);
    let snapshot = {
        let state = replica.state.lock().unwrap();
        let snapshot = state.snapshot.as_ref().unwrap();
        if snapshot.token.len() > manager.inner.settings.snapshot_bytes.get() {
            return Err(invalid("source snapshot exceeds the configured bound"));
        }
        snapshot.clone()
    };
    let view = ReadView::new(id, through.clone(), through.clone(), snapshot)?;
    let mut schemas = Cursor::<StorageSchema>::new();
    let mut records = Cursor::<StoredRecord>::new();
    let mut rows = Cursor::<StoredRow>::new();
    let mut files = Cursor::<FileReference>::new();
    let mut objects = Cursor::<ObjectEntry>::new();
    let mut copied = after.sequence;
    loop {
        executor::yield_now().await;
        schemas
            .fill(manager, replica, &view, after.sequence)
            .await?;
        records
            .fill(manager, replica, &view, after.sequence)
            .await?;
        rows.fill(manager, replica, &view, after.sequence).await?;
        files.fill(manager, replica, &view, after.sequence).await?;
        objects
            .fill(manager, replica, &view, after.sequence)
            .await?;
        let heads = [
            schemas.first(),
            records.first(),
            rows.first(),
            files.first(),
            objects.first(),
        ];
        let next = heads
            .into_iter()
            .enumerate()
            .filter_map(|(kind, position)| position.map(|position| (position, kind)))
            .min();
        let Some((mut position, kind)) = next else {
            if copied != through.sequence {
                return Err(invalid("source has a gap before its saved boundary"));
            }
            return Ok(());
        };
        if copied.checked_add(1) != Some(position) {
            return Err(invalid("source has a missing or repeated capture position"));
        }
        if heads
            .into_iter()
            .flatten()
            .filter(|value| *value == position)
            .count()
            != 1
        {
            return Err(invalid("source has a repeated capture position"));
        }
        let before = heads
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index != kind)
            .filter_map(|(_, position)| position)
            .min();
        replica.check_copying()?;
        match kind {
            0 => {
                let entries = schemas.batch(before, copied)?;
                position = entries.last().unwrap().position().sequence;
                write(manager, replica, StoragePart::Schemas(&entries)).await?;
            }
            1 => {
                let entries = records.batch(before, copied)?;
                position = entries.last().unwrap().position().sequence;
                write(manager, replica, StoragePart::Records(&entries)).await?;
            }
            2 => {
                let entries = rows.batch(before, copied)?;
                position = entries.last().unwrap().position().sequence;
                write(manager, replica, StoragePart::Stream(&entries)).await?;
            }
            3 => {
                let entry = files.entries.pop_front().unwrap();
                file(manager, replica, &view, &entry, None).await?;
                write(
                    manager,
                    replica,
                    StoragePart::Files(std::slice::from_ref(&entry)),
                )
                .await?;
            }
            4 => {
                let entries = objects.batch(before, copied)?;
                position = entries.last().unwrap().position().sequence;
                write(manager, replica, StoragePart::Objects(&entries)).await?;
            }
            _ => unreachable!("five storage collections"),
        }
        copied = position;
    }
}

pub(super) async fn file(
    manager: &StorageManager<crate::storage::ErasedCatalog>,
    replica: &Replica,
    view: &ReadView,
    file: &FileReference,
    bootstrap: Option<&BootstrapId>,
) -> Result<()> {
    let staging = StagedFile {
        history: file.position.history.clone(),
        id: file.metadata.id.clone(),
    };
    replica.state.lock().unwrap().staging = Some(super::replication::FileStaging {
        file: staging.clone(),
        bootstrap: bootstrap.cloned(),
    });
    let creation = match bootstrap {
        Some(identity) => replica
            .config
            .backend
            .create_bootstrap_file(identity, &file.metadata),
        None => replica.config.backend.create_file(&file.metadata),
    };
    let actual = manager.inner.bounded(creation).await?;
    if actual != staging {
        replica.state.lock().unwrap().extra_staging = Some(super::replication::FileStaging {
            file: actual,
            bootstrap: bootstrap.cloned(),
        });
        return Err(invalid("target returned a different staging identity"));
    }
    let mut offset = 0;
    let mut digest = Sha256::new();
    while offset < file.length {
        executor::yield_now().await;
        replica.check_copying()?;
        let maximum = NonZeroUsize::new(
            (file.length - offset).min(manager.inner.settings.file_chunk_bytes.get() as u64)
                as usize,
        )
        .unwrap();
        let bytes = manager
            .inner
            .bounded(manager.inner.primary.read_file(view, file, offset, maximum))
            .await?;
        if bytes.is_empty() || bytes.capacity() > maximum.get() {
            return Err(invalid("source file length is inconsistent"));
        }
        replica.check_copying()?;
        let write = match bootstrap {
            Some(identity) => replica
                .config
                .backend
                .write_bootstrap_file(identity, &staging, offset, &bytes),
            None => replica.config.backend.write_file(&staging, offset, &bytes),
        };
        manager.inner.bounded(write).await?;
        digest.update(&bytes);
        offset += bytes.len() as u64;
    }
    if <[u8; 32]>::from(digest.finalize()) != file.sha256 {
        return Err(invalid("source file digest is inconsistent"));
    }
    replica.check_copying()?;
    let finish = match bootstrap {
        Some(identity) => replica.config.backend.finish_bootstrap_file(
            identity,
            &staging,
            &file.position,
            file.length,
            file.sha256,
        ),
        None => {
            replica
                .config
                .backend
                .finish_file(&staging, &file.position, file.length, file.sha256)
        }
    };
    let actual = manager.inner.bounded(finish).await?;
    if actual != *file {
        return Err(invalid("target changed the immutable file reference"));
    }
    // A replay may have created staging even if a prior uncertain finish won.
    // Abort is idempotent and cannot delete the completed immutable file.
    match bootstrap {
        Some(identity) => {
            replica
                .config
                .backend
                .abort_bootstrap_file(identity, &staging)
                .await?
        }
        None => replica.config.backend.abort_file(&staging).await?,
    }
    replica.state.lock().unwrap().staging = None;
    Ok(())
}

/* === Private Functions === */
async fn write(
    manager: &StorageManager<crate::storage::ErasedCatalog>,
    replica: &Replica,
    part: StoragePart<'_>,
) -> Result<()> {
    let parts = [part];
    let end = part.entries().last().unwrap().position().sequence;
    let mut after = part.entries().next().unwrap().position().sequence - 1;
    while after < end {
        replica.check_copying()?;
        let batch = StorageBatch::prefix(&parts, after, manager.inner.settings.max_batch_bytes)?;
        let through = batch.through();
        let receipt = manager
            .inner
            .bounded(replica.config.backend.write(batch))
            .await?;
        if receipt != *through {
            return Err(invalid("replica returned a different saved boundary"));
        }
        after = receipt.sequence;
        replica.state.lock().unwrap().status.saved = Some(receipt);
        manager.inner.notify();
    }
    Ok(())
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "replication transfer",
        reason,
    }
}

/* === Tests === */
