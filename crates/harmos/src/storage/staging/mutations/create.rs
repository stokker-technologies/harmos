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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn create_file_once(
        &self,
        mutation: &MutationId,
        metadata: &FileMetadata,
    ) -> Result<FileProducer> {
        let mut hash = Sha256::new();
        hash_part(&mut hash, b"storage.create_file:v1");
        hash_part(&mut hash, metadata.id.as_str().as_bytes());
        hash.update([u8::from(metadata.scope.is_some())]);
        if let Some(scope) = &metadata.scope {
            hash_part(&mut hash, scope.as_str().as_bytes());
        }
        hash_part(&mut hash, metadata.media_type.as_bytes());
        hash.update([u8::from(metadata.expected_length.is_some())]);
        if let Some(length) = metadata.expected_length {
            hash.update(length.to_le_bytes());
        }
        hash.update([u8::from(metadata.expected_sha256.is_some())]);
        if let Some(digest) = metadata.expected_sha256 {
            hash.update(digest);
        }
        let fingerprint = hash.finalize().into();
        self.mutate_file(mutation, fingerprint, |state| {
            if !self.inner.primary.supports_file_sessions() {
                return Err(invalid("backend does not support resumable production"));
            }
            let (reference_bytes, base_bytes) = file_bytes(metadata, self.inner.primary.history())?;
            if reference_bytes > self.inner.settings.max_batch_bytes.get() {
                return Err(invalid("reference exceeds max_batch_bytes"));
            }
            if state
                .files
                .values()
                .any(|file| file.metadata.id == metadata.id)
                || state.deleting(&metadata.id)
            {
                return Err(Error::AlreadyRegistered {
                    domain: "storage file",
                    name: metadata.id.to_string(),
                });
            }
            if state.files.len() >= self.inner.settings.max_files.get() {
                return Err(Error::ResourceExhausted {
                    resource: "file producers",
                });
            }
            let outcome_bytes = std::mem::size_of::<MutationValue>()
                .saturating_add(self.inner.primary.history().memory_size())
                .saturating_add(metadata.id.memory_size());
            let id = NEXT_FILE
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| size_error())?;
            reserve(
                state,
                mutation,
                fingerprint,
                outcome_bytes,
                base_bytes.checked_add(1).ok_or_else(size_error)?,
                &self.inner.settings,
            )?;
            state.files.insert(
                id,
                FileState {
                    metadata: Arc::new(metadata.clone()),
                    staging: StagedFile {
                        history: self.inner.primary.history().clone(),
                        id: metadata.id.clone(),
                    },
                    session: Some(Session::new(self.inner.settings.file_session_timeout)),
                    mutation: Some(Mutation {
                        id: mutation.clone(),
                        kind: Kind::Create,
                    }),
                    extra: None,
                    progress: Arc::new(Mutex::new(Progress {
                        generation: 1,
                        ..Progress::default()
                    })),
                    hash: Sha256::new(),
                    length: 0,
                    ticket: 0,
                    command: Some(Command::Create),
                    abandoned: false,
                    in_flight: false,
                    retrying: false,
                    base_bytes,
                    reference_bytes,
                },
            );
            Ok(true)
        })
        .await?
        .producer()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
