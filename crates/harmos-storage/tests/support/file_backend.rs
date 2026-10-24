/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve Staged File And Mutation Persistence.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: ObjectCatalog> Data<C> {
    pub(super) fn fixture_supports_file_sessions(&self) -> bool {
        true
    }

    pub(super) fn fixture_supports_mutation_receipts(&self) -> bool {
        true
    }

    pub(super) fn fixture_save_mutations<'a>(&'a self, receipts: &'a [MutationReceipt]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut saved = self.mutation_receipts.lock().unwrap();
            let now = std::time::SystemTime::now();
            saved.retain(|_, receipt| receipt.expires_at > now);
            for receipt in receipts {
                if receipt.expires_at <= now {
                    continue;
                }
                if saved.get(&receipt.mutation).is_some_and(|prior| prior != receipt) {
                    return Err(failure(BackendFailure::Conflict));
                }
                saved.insert(receipt.mutation.clone(), receipt.clone());
            }
            if self.control.receipt_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }

    pub(super) fn fixture_recover_mutations(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        Box::pin(async move {
            let mut saved = self.mutation_receipts.lock().unwrap();
            saved.retain(|_, receipt| receipt.expires_at > std::time::SystemTime::now());
            let mut newest: Vec<_> = saved.values().collect();
            newest.sort_by_key(|receipt| std::cmp::Reverse(receipt.expires_at));
            let mut bytes = std::mem::size_of::<Vec<MutationReceipt>>();
            let mut receipts = Vec::new();
            for receipt in newest.into_iter().take(limits.rows.get()) {
                bytes = bytes.saturating_add(receipt.memory_size());
                if bytes > limits.bytes.get() {
                    break;
                }
                receipts.push(receipt.clone());
            }
            Ok(receipts)
        })
    }

    pub(super) fn fixture_create_production<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            if self.staged.lock().unwrap().contains_key(metadata.id.as_str())
                && !self.resumable_files.lock().unwrap().contains(metadata.id.as_str())
            {
                return Err(failure(BackendFailure::Conflict));
            }
            if self.blobs.lock().unwrap().values().any(|(file, _)| file.metadata.id == metadata.id) {
                return Err(failure(BackendFailure::Conflict));
            }
            self.resumable_files.lock().unwrap().insert(metadata.id.to_string());
            self.file_generations.lock().unwrap().entry(metadata.id.to_string()).or_insert(1);
            self.create_file(metadata).await
        })
    }

    pub(super) fn fixture_fence_production<'a>(&'a self, file: &'a StagedFile, expected: u64, next: u64) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.control.block_fence.load(Ordering::SeqCst) {
                self.control.fence_entered.notify_one();
                self.control.fence_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            self.history.check(&file.history)?;
            if expected.checked_add(1) != Some(next) {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut generations = self.file_generations.lock().unwrap();
            let generation = generations.get_mut(file.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
            if *generation != expected && *generation != next {
                return Err(failure(BackendFailure::Conflict));
            }
            *generation = next;
            if self.control.fence_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            self.control.fenced.notify_one();
            Ok(())
        })
    }

    pub(super) fn fixture_save_production<'a>(&'a self, checkpoint: &'a FileCheckpoint) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.control.block_checkpoint.load(Ordering::SeqCst) {
                self.control.checkpoint_entered.notify_one();
                self.control.checkpoint_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            self.history.check(&checkpoint.session.history)?;
            let id = checkpoint.session.id.as_str();
            if self.deleted_files.lock().unwrap().contains(id)
                || self.abandoned_files.lock().unwrap().contains(id)
                || self.blobs.lock().unwrap().values().any(|(file, _)| file.metadata.id == checkpoint.session.id)
            {
                return Ok(());
            }
            if self.file_generations.lock().unwrap().get(id).is_none_or(|generation| *generation < checkpoint.session.generation) {
                return Err(failure(BackendFailure::Conflict));
            }
            let staged = self.staged.lock().unwrap();
            let (metadata, bytes) = staged.get(id).ok_or_else(|| failure(BackendFailure::Permanent))?;
            let length = usize::try_from(checkpoint.length).map_err(|_| failure(BackendFailure::Corrupt))?;
            let prefix = bytes.get(..length).ok_or_else(|| failure(BackendFailure::Corrupt))?;
            if *metadata != checkpoint.metadata || <[u8; 32]>::from(Sha256::digest(prefix)) != checkpoint.sha256()? {
                return Err(failure(BackendFailure::Corrupt));
            }
            let mut saved = self.saved_productions.lock().unwrap();
            if let Some((prior, prior_bytes)) = saved.get(id) {
                if prior.length > checkpoint.length {
                    return Ok(());
                }
                if prior.length == checkpoint.length && prior_bytes != prefix {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            saved.insert(id.to_string(), (checkpoint.clone(), prefix.to_vec()));
            if self.control.checkpoint_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }

    pub(super) fn fixture_recover_productions(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        Box::pin(async move {
            let _io = self.file_io.lock().unwrap();
            let saved = self.saved_productions.lock().unwrap();
            let required = std::mem::size_of::<Vec<FileCheckpoint>>()
                .saturating_add(saved.values().map(|(checkpoint, _)| checkpoint.memory_size()).sum::<usize>());
            if saved.len() > limits.rows.get() || required > limits.bytes.get() {
                return Err(failure(BackendFailure::Permanent));
            }
            let mut restored = Vec::with_capacity(saved.len());
            for (id, (checkpoint, bytes)) in saved.iter() {
                if <[u8; 32]>::from(Sha256::digest(bytes)) != checkpoint.sha256()? || bytes.len() as u64 != checkpoint.length {
                    return Err(failure(BackendFailure::Corrupt));
                }
                let mut checkpoint = checkpoint.clone();
                checkpoint.session.generation =
                    *self.file_generations.lock().unwrap().get(id).ok_or_else(|| failure(BackendFailure::Corrupt))?;
                self.staged.lock().unwrap().insert(id.clone(), (checkpoint.metadata.clone(), bytes.clone()));
                restored.push(checkpoint);
            }
            // Unsaved Productions Have No Durable Resume Boundary.
            for id in self.resumable_files.lock().unwrap().iter() {
                if !saved.contains_key(id) && self.staged.lock().unwrap().contains_key(id) {
                    self.abandoned_files.lock().unwrap().insert(id.clone());
                }
            }
            Ok(restored)
        })
    }

    pub(super) fn fixture_file_progress<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        Box::pin(async move {
            let _io = self.file_io.lock().unwrap();
            self.history.check(&file.history)?;
            if let Some((reference, _)) = self.blobs.lock().unwrap().values().find(|(reference, _)| reference.metadata.id == file.id) {
                return Ok(FileProgress { length: reference.length, phase: FilePhase::Finished });
            }
            if !self.resumable_files.lock().unwrap().contains(file.id.as_str()) {
                return Err(failure(BackendFailure::Permanent));
            }
            let staged = self.staged.lock().unwrap();
            let (_, bytes) = staged.get(file.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
            let phase =
                if self.abandoned_files.lock().unwrap().contains(file.id.as_str()) { FilePhase::Abandoned } else { FilePhase::Producing };
            Ok(FileProgress { length: bytes.len() as u64, phase })
        })
    }

    pub(super) fn fixture_read_production<'a>(
        &'a self, file: &'a StagedFile, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.history.check(&file.history)?;
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            let copy = |bytes: &[u8]| -> Result<Vec<u8>> {
                let offset = usize::try_from(offset).map_err(|_| failure(BackendFailure::Permanent))?;
                let end = bytes.len().min(offset.saturating_add(max_bytes.get()));
                let chunk = bytes.get(offset..end).ok_or_else(|| failure(BackendFailure::Permanent))?;
                Ok(Box::<[u8]>::from(chunk).into_vec())
            };
            if let Some((_, bytes)) = self.blobs.lock().unwrap().values().find(|(reference, _)| reference.metadata.id == file.id) {
                return copy(bytes);
            }
            if !self.resumable_files.lock().unwrap().contains(file.id.as_str()) {
                return Err(failure(BackendFailure::Permanent));
            }
            let staged = self.staged.lock().unwrap();
            let (_, bytes) = staged.get(file.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
            copy(bytes)
        })
    }

    pub(super) fn fixture_supports_file_deletion(&self) -> bool {
        true
    }

    pub(super) fn fixture_delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&file.history)?;
            if self.control.block_delete.load(Ordering::SeqCst) {
                self.control.delete_entered.notify_one();
                self.control.delete_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            let id = file.id.as_str();
            let mut staged = self.staged.lock().unwrap();
            if staged.contains_key(id) && !self.abandoned_files.lock().unwrap().contains(id) {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut files = self.files.lock().unwrap();
            let mut deleted = self.deleted_files.lock().unwrap();
            if deleted.insert(id.to_owned()) {
                let last =
                    files.values().filter(|value| value.metadata.id == file.id).map(|value| value.position.sequence).max().unwrap_or(0);
                self.lost_files_through.fetch_max(last as usize, Ordering::SeqCst);
            }
            files.retain(|_, value| value.metadata.id != file.id);
            staged.remove(id);
            self.blobs.lock().unwrap().remove(id);
            self.saved_productions.lock().unwrap().remove(id);
            if self.control.delete_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }

    pub(super) fn fixture_abandon_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.control.block_abort.load(Ordering::SeqCst) {
                self.control.abort_entered.notify_one();
                self.control.abort_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            self.history.check(&staging.history)?;
            if self.blobs.lock().unwrap().values().any(|(file, _)| file.metadata.id == staging.id) {
                return Ok(());
            }
            if self.control.fail_abort.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.saved_productions.lock().unwrap().remove(staging.id.as_str());
            self.abandoned_files.lock().unwrap().insert(staging.id.to_string());
            if self.control.abandon_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            self.control.abandoned.notify_one();
            Ok(())
        })
    }

    pub(super) fn fixture_create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            if self.abandoned_files.lock().unwrap().contains(metadata.id.as_str()) {
                return Err(failure(BackendFailure::Conflict));
            }
            {
                let _io = self.file_io.lock().unwrap();
                if self.deleted_files.lock().unwrap().contains(metadata.id.as_str()) {
                    return Err(failure(BackendFailure::Conflict));
                }
                let mut files = self.staged.lock().unwrap();
                let existing = files.entry(metadata.id.to_string()).or_insert_with(|| (metadata.clone(), Vec::new()));
                if existing.0 != *metadata {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            if self.control.block_create.load(Ordering::SeqCst) {
                self.control.create_entered.notify_one();
                self.control.create_release.notified().await;
            }
            if self.control.fail_create.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            if self.control.create_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            if let Some(journal) = &self.journal {
                journal.stage(&metadata.id, &[])?;
            }
            Ok(StagedFile { history: self.history.clone(), id: metadata.id.clone() })
        })
    }

    pub(super) fn fixture_write_file<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: &'a [u8]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.abandoned_files.lock().unwrap().contains(staging.id.as_str()) {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control.chunk_sizes.lock().unwrap().push(bytes.len());
            self.control.chunk_offsets.lock().unwrap().push(offset);
            if self.control.block_chunk.load(Ordering::SeqCst) {
                self.control.chunk_entered.notify_one();
                self.control.chunk_release.notified().await;
            }
            {
                let _io = self.file_io.lock().unwrap();
                let mut files = self.staged.lock().unwrap();
                let (_, stored) = files.get_mut(staging.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
                let offset = usize::try_from(offset).unwrap();
                if offset == stored.len() {
                    stored.extend_from_slice(bytes);
                } else if stored.get(offset..offset + bytes.len()) != Some(bytes) {
                    return Err(failure(BackendFailure::Conflict));
                }
                if let Some(journal) = &self.journal {
                    journal.stage(&staging.id, stored)?;
                }
            }
            if self.control.chunk_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }

    pub(super) fn fixture_finish_file<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            if self.abandoned_files.lock().unwrap().contains(staging.id.as_str()) {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control.finish_positions.lock().unwrap().push(position.sequence);
            if self.control.block_finish.load(Ordering::SeqCst) {
                self.control.finish_entered.notify_one();
                self.control.finish_release.notified().await;
            }
            if self.control.fail_finish.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            let _io = self.file_io.lock().unwrap();
            if let Some((file, _)) = self.blobs.lock().unwrap().get(staging.id.as_str()) {
                if file.position != *position || file.length != length || file.sha256 != sha256 || file.metadata.id != staging.id {
                    return Err(failure(BackendFailure::Conflict));
                }
                return Ok(file.clone());
            }
            let mut staged = self.staged.lock().unwrap();
            let (_, bytes) = staged.get(staging.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
            if bytes.len() as u64 != length || <[u8; 32]>::from(Sha256::digest(bytes)) != sha256 {
                return Err(failure(BackendFailure::Corrupt));
            }
            if let Some(journal) = &self.journal {
                journal.finish(&staging.id, bytes, &sha256)?;
            }
            // A Failed Integrity Check Must Retain The Last Recoverable Prefix.
            let (metadata, bytes) = staged.remove(staging.id.as_str()).expect("verified staging");
            self.saved_productions.lock().unwrap().remove(staging.id.as_str());
            let reference = FileReference { position: position.clone(), metadata, length, sha256 };
            self.blobs.lock().unwrap().insert(staging.id.to_string(), (reference.clone(), bytes));
            if self.control.finish_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(reference)
        })
    }

    pub(super) fn fixture_abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.aborts.fetch_add(1, Ordering::SeqCst);
            if self.control.block_abort.load(Ordering::SeqCst) {
                self.control.abort_entered.notify_one();
                self.control.abort_release.notified().await;
            }
            if self.control.fail_abort.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.staged.lock().unwrap().remove(staging.id.as_str());
            Ok(())
        })
    }

    pub(super) fn fixture_read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            self.image(snapshot)?;
            let mut result = {
                let blobs = self.blobs.lock().unwrap();
                let (actual, bytes) = blobs.get(file.metadata.id.as_str()).ok_or_else(|| failure(BackendFailure::Permanent))?;
                if actual != file {
                    return Err(failure(BackendFailure::Conflict));
                }
                let offset = usize::try_from(offset).unwrap();
                let end = bytes.len().min(offset.saturating_add(max_bytes.get()));
                bytes[offset..end].to_vec()
            };
            if self.control.corrupt_file.load(Ordering::SeqCst) && !result.is_empty() {
                result[0] ^= 1;
            }
            let accepted = self.control.accepted_file_read.lock().unwrap().take();
            if let Some((started, release)) = accepted {
                started.send(()).unwrap();
                release.await.unwrap();
            }
            Ok(result)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
