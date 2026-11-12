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

impl StorageManager {
    #[tracing::instrument(name = "storage.staging.mutations.append.append_file_once", level = "debug", skip_all)]
    pub async fn append_file_once(
        &self, mutation: &MutationId, token: &FileSession, offset: u64, bytes: impl Into<crate::FileChunk>,
    ) -> Result<FileProducer> {
        let bytes = bytes.into();
        self.inner.primary.get_history().check(&token.history)?;
        let mut hash = Sha256::new();
        hash_part(&mut hash, b"storage.append_file:v1");
        hash_session(&mut hash, token);
        hash.update(offset.to_le_bytes());
        hash_part(&mut hash, bytes.as_slice());
        let fingerprint = hash.finalize().into();
        let mut bytes = Some(bytes);
        self.mutate_file(mutation, fingerprint, |state| {
            let payload = bytes.as_ref().expect("unadmitted file payload");
            if payload.is_empty() || payload.len() > self.inner.settings.response.batch_bytes.get() { return Err(invalid("append requires a nonempty bounded batch")); }
            let (id, file) = active(state, token)?;
            file.check_session(Some(token))?;
            if file.command.is_some() || file.in_flight { return Ok(false); }
            let length = offset.checked_add(payload.len() as u64).ok_or_else(size_error)?;
            if offset > file.length || (offset < file.length && length > file.length) {
                tracing::error!(file = %token.id, generation = token.generation, offset, actual = file.length, reason = "append offset mismatch", "file append rejected");
                return Err(invalid("append offset differs from acknowledged length"));
            }
            let replay = offset < file.length;
            if file.metadata.expected_length.is_some_and(|expected| length > expected) { return Err(invalid("append exceeds expected file length")); }
            let ticket = file.ticket.checked_add(1).ok_or_else(size_error)?;
            reserve(state, mutation, fingerprint, payload.len(), &self.inner.settings)?;
            let file = state.files.get_mut(&id).expect("selected file");
            file.ticket = ticket;
            file.context = crate::execution::Context::current();
            file.queued = Some(tracing::debug_span!("storage.file.queue"));
            let payload = bytes.take().expect("unadmitted file payload");
            file.command = Some(if replay { Command::Replay { offset, bytes: payload } } else { Command::Write(payload) });
            file.mutation = Some(Mutation { id: mutation.clone(), kind: Kind::Append });
            Ok(true)
        }).await?.producer().map_err(|_| Error::backend("AppendFileBytes", crate::BackendFailure::Conflict, std::io::Error::other("bytes differ from the accepted prefix")))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
