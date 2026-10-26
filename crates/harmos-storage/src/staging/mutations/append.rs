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
    pub async fn append_file_once(
        &self, mutation: &MutationId, token: &FileSession, offset: u64, bytes: impl Into<crate::FileChunk>,
    ) -> Result<FileProducer> {
        let bytes = bytes.into();
        self.inner.primary.history().check(&token.history)?;
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
            if file.length != offset {
                tracing::error!(file = %token.id, generation = token.generation, offset, actual = file.length, reason = "append offset mismatch", "file append rejected");
                return Err(invalid("append offset differs from acknowledged length"));
            }
            let length = offset.checked_add(payload.len() as u64).ok_or_else(size_error)?;
            if file.metadata.expected_length.is_some_and(|expected| length > expected) { return Err(invalid("append exceeds expected file length")); }
            let ticket = file.ticket.checked_add(1).ok_or_else(size_error)?;
            let outcome = MutationValue::Producer(FileProducer { session: token.clone(), length });
            reserve(state, mutation, fingerprint, outcome.memory_size(), payload.capacity(), &self.inner.settings)?;
            let file = state.files.get_mut(&id).expect("selected file");
            file.ticket = ticket;
            let payload = bytes.take().expect("unadmitted file payload");
            let charge = crate::batch::WriteCharge::new(&self.inner, payload.allocation_bytes());
            file.command = Some(Command::Write(payload.retained(charge)));
            file.mutation = Some(Mutation { id: mutation.clone(), kind: Kind::Append });
            Ok(true)
        }).await?.producer()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
