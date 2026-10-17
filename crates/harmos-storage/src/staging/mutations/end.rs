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
    pub async fn finish_file_once(
        &self,
        mutation: &MutationId,
        token: &FileSession,
    ) -> Result<FileReference> {
        self.inner.primary.history().check(&token.history)?;
        let fingerprint = fingerprint("storage.finish_file:v1", token);
        self.mutate_file(mutation, fingerprint, |state| {
            let (id, file) = active(state, token)?;
            file.check_session(Some(token))?;
            if file.command.is_some() || file.in_flight { return Ok(false); }
            let digest: [u8; 32] = file.hash.clone().finalize().into();
            let valid = file.metadata.expected_length.is_none_or(|length| length == file.length) && file.metadata.expected_sha256.is_none_or(|expected| expected == digest);
            let bytes = file.base_bytes.checked_add(std::mem::size_of::<MutationValue>()).ok_or_else(size_error)?;
            reserve(state, mutation, fingerprint, bytes, 0, &self.inner.settings)?;
            let file = state.files.get_mut(&id).expect("selected file");
            file.mutation = Some(Mutation { id: mutation.clone(), kind: Kind::Finish });
            if valid { file.command = Some(Command::Finish { target: None, sha256: digest }); } else {
                tracing::error!(file = %token.id, generation = token.generation, offset = file.length, reason = "length or digest mismatch", "file finish rejected");
                file.abandoned = true;
                file.session.as_mut().expect("managed producer").reason = "file integrity mismatch";
                file.command = Some(Command::Abort);
                file.progress.lock().unwrap().closed = true;
            }
            Ok(true)
        }).await?.reference()
    }

    pub async fn abandon_file_once(
        &self,
        mutation: &MutationId,
        token: &FileSession,
    ) -> Result<FileProgress> {
        self.inner.primary.history().check(&token.history)?;
        let fingerprint = fingerprint("storage.abandon_file:v1", token);
        self.mutate_file(mutation, fingerprint, |state| {
            let (id, file) = active(state, token)?;
            file.check_session(Some(token))?;
            if matches!(file.command, Some(Command::Finish { .. })) {
                return Err(invalid("finish is already being reconciled"));
            }
            if file.command.is_some() || file.in_flight {
                return Ok(false);
            }
            reserve(
                state,
                mutation,
                fingerprint,
                std::mem::size_of::<MutationValue>(),
                0,
                &self.inner.settings,
            )?;
            let file = state.files.get_mut(&id).expect("selected file");
            file.mutation = Some(Mutation {
                id: mutation.clone(),
                kind: Kind::Abandon,
            });
            file.abandoned = true;
            file.session.as_mut().expect("managed producer").reason =
                "producer explicitly abandoned";
            file.command = Some(Command::Abort);
            file.progress.lock().unwrap().closed = true;
            Ok(true)
        })
        .await?
        .progress()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
