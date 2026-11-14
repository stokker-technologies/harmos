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

/* === Public Functions === */

pub(super) fn create(state: &State, metadata: &FileMetadata) -> Result<StagedFile> {
    state.check()?;
    let data = encode(metadata)?;
    if data.len() > state.settings.chunk_bytes.get() {
        return Err(invalid("file metadata exceeds chunk budget"));
    }
    let prior: Option<(Vec<u8>, Option<Vec<u8>>)> = state
        .connection
        .query_row("SELECT metadata,finished FROM storage_files WHERE id=?1", [metadata.id.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
        .map_err(db)?;
    if prior.as_ref().is_some_and(|(prior, finished)| prior != &data || finished.is_some()) {
        return Err(conflict("file identity is already in use"));
    }
    state
        .connection
        .execute("INSERT OR IGNORE INTO storage_files VALUES(?1,?2,0,NULL)", params![metadata.id.as_str(), data])
        .map_err(db)?;
    Ok(StagedFile {
        history: state.history.clone(),
        id: metadata.id.clone(),
    })
}

pub(super) fn write(state: &mut State, file: &StagedFile, offset: u64, bytes: &[u8]) -> Result<()> {
    state.check()?;
    state.history.check(&file.history)?;
    let abandoned: bool = state
        .connection
        .query_row("SELECT EXISTS(SELECT 1 FROM storage_file_sessions WHERE id=?1 AND abandoned=1)", [file.id.as_str()], |row| row.get(0))
        .map_err(db)?;
    if abandoned {
        return Err(conflict("append file bytes: production is abandoned"));
    }
    if bytes.len() > state.settings.chunk_bytes.get() {
        return Err(invalid("file chunk exceeds chunk budget"));
    }
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let (length, finished): (u64, Option<Vec<u8>>) = tx
        .query_row("SELECT length,finished FROM storage_files WHERE id=?1", [file.id.as_str()], |row| Ok((unsigned(row, 0)?, row.get(1)?)))
        .map_err(db)?;
    if finished.is_some() {
        return Err(conflict("file is already finished"));
    }
    let prior: Option<Vec<u8>> = tx
        .query_row("SELECT data FROM storage_file_chunks WHERE id=?1 AND offset=?2", params![file.id.as_str(), integer(offset)?], |row| {
            row.get(0)
        })
        .optional()
        .map_err(db)?;
    if let Some(prior) = prior {
        if prior != bytes {
            return Err(conflict("replayed file chunk differs"));
        }
        return Ok(());
    }
    if offset != length {
        return Err(conflict("file chunks must append contiguously"));
    }
    let end = offset.checked_add(bytes.len() as u64).ok_or_else(|| invalid("file length overflow"))?;
    if !bytes.is_empty() {
        tx.execute("INSERT INTO storage_file_chunks VALUES(?1,?2,?3)", params![file.id.as_str(), integer(offset)?, bytes]).map_err(db)?;
    }
    tx.execute("UPDATE storage_files SET length=?1 WHERE id=?2", params![integer(end)?, file.id.as_str()]).map_err(db)?;
    tx.commit().map_err(db)
}

pub(super) fn finish(
    state: &mut State, file: &StagedFile, position: &StorageCursor, length: u64, sha256: [u8; 32],
) -> Result<FileReference> {
    state.check()?;
    state.history.check(&file.history)?;
    state.history.check(&position.history)?;
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let (metadata, actual, finished): (Vec<u8>, u64, Option<Vec<u8>>) = tx
        .query_row("SELECT metadata,length,finished FROM storage_files WHERE id=?1", [file.id.as_str()], |row| {
            Ok((row.get(0)?, unsigned(row, 1)?, row.get(2)?))
        })
        .map_err(db)?;
    let metadata: FileMetadata = decode(&metadata)?;
    let reference = FileReference {
        position: position.clone(),
        metadata,
        length,
        sha256,
    };
    if let Some(finished) = finished {
        if decode::<FileReference>(&finished)? != reference {
            return Err(conflict("finished file replay differs"));
        }
        return Ok(reference);
    }
    if actual != length
        || reference.metadata.expected_length.is_some_and(|expected| expected != length)
        || reference.metadata.expected_sha256.is_some_and(|expected| expected != sha256)
    {
        return Err(conflict("file completion metadata differs"));
    }
    let mut hash = Sha256::new();
    {
        let mut query = tx.prepare("SELECT offset,data FROM storage_file_chunks WHERE id=?1 ORDER BY offset").map_err(db)?;
        let mut chunks = query.query([file.id.as_str()]).map_err(db)?;
        let mut through = 0;
        while let Some(chunk) = chunks.next().map_err(db)? {
            let offset = unsigned(chunk, 0).map_err(db)?;
            let bytes = chunk.get_ref(1).map_err(db)?.as_blob().map_err(|_| incompatible())?;
            if offset != through || bytes.len() > state.settings.chunk_bytes.get() {
                return Err(incompatible());
            }
            through += bytes.len() as u64;
            hash.update(bytes);
        }
        if through != length || <[u8; 32]>::from(hash.finalize()) != sha256 {
            return Err(conflict("file digest differs"));
        }
    }
    tx.execute("UPDATE storage_files SET finished=?1 WHERE id=?2", params![encode(&reference)?, file.id.as_str()]).map_err(db)?;
    tx.commit().map_err(db)?;
    Ok(reference)
}

pub(super) fn abort(state: &mut State, file: &StagedFile) -> Result<()> {
    state.check()?;
    state.history.check(&file.history)?;
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let finished: Option<Vec<u8>> = tx
        .query_row("SELECT finished FROM storage_files WHERE id=?1", [file.id.as_str()], |row| row.get(0))
        .optional()
        .map_err(db)?
        .flatten();
    if finished.is_some() {
        return Err(conflict("finished file cannot be aborted"));
    }
    tx.execute("DELETE FROM storage_file_chunks WHERE id=?1", [file.id.as_str()]).map_err(db)?;
    tx.execute("DELETE FROM storage_file_sessions WHERE id=?1", [file.id.as_str()]).map_err(db)?;
    tx.execute("DELETE FROM storage_files WHERE id=?1", [file.id.as_str()]).map_err(db)?;
    tx.commit().map_err(db)
}

pub(super) fn read(state: &State, snapshot: &BackendSnapshot, file: &FileReference, offset: u64, maximum: NonZeroUsize) -> Result<Vec<u8>> {
    state.snapshot(snapshot)?;
    state.history.check(&file.position.history)?;
    if file.position.sequence > snapshot.at.sequence {
        return Err(invalid("file is beyond the snapshot"));
    }
    let published: bool = state
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM storage_rows WHERE kind=4 AND position=?1 AND data=?2)",
            params![integer(file.position.sequence)?, encode(file)?],
            |row| row.get(0),
        )
        .map_err(db)?;
    if !published {
        return Err(conflict("file reference is not published"));
    }
    let finished: Vec<u8> = state
        .connection
        .query_row("SELECT finished FROM storage_files WHERE id=?1", [file.metadata.id.as_str()], |row| row.get(0))
        .map_err(db)?;
    if decode::<FileReference>(&finished)? != *file {
        return Err(conflict("file reference differs"));
    }
    let end = offset.saturating_add(maximum.get() as u64).min(file.length);
    if offset >= end {
        return Ok(Vec::new());
    }
    bytes(
        state,
        &StagedFile {
            history: file.position.history.clone(),
            id: file.metadata.id.clone(),
        },
        offset,
        maximum,
        file.length,
    )
}

pub(super) fn bytes(state: &State, file: &StagedFile, offset: u64, maximum: NonZeroUsize, length: u64) -> Result<Vec<u8>> {
    let end = offset.saturating_add(maximum.get() as u64).min(length);
    if offset >= end {
        return Ok(Vec::new());
    }
    let mut query = state.connection.prepare("SELECT substr(data,max(0,?2-offset)+1,min(length(data),?3-offset)-max(0,?2-offset)) FROM storage_file_chunks WHERE id=?1 AND offset<?3 AND offset+length(data)>?2 ORDER BY offset").map_err(db)?;
    let chunks = query.query_map(params![file.id.as_str(), integer(offset)?, integer(end)?], |row| row.get::<_, Vec<u8>>(0)).map_err(db)?;
    let mut output = Vec::with_capacity((end - offset) as usize);
    for chunk in chunks {
        output.extend(chunk.map_err(db)?);
    }
    if output.len() as u64 != end - offset {
        return Err(incompatible());
    }
    Ok(output)
}

pub(super) fn production(state: &State, metadata: &FileMetadata) -> Result<StagedFile> {
    let file = create(state, metadata)?;
    state.connection.execute("INSERT OR IGNORE INTO storage_file_sessions VALUES(?1,1,0,NULL)", [file.id.as_str()]).map_err(db)?;
    if progress(state, &file)?.phase != FilePhase::Producing {
        return Err(conflict("create file production: identity is no longer producing"));
    }
    Ok(file)
}

pub(super) fn fence(state: &State, file: &StagedFile, expected: u64, next: u64) -> Result<()> {
    state.check()?;
    state.history.check(&file.history)?;
    if expected == 0 || next <= expected {
        return Err(invalid("fence file writer: generation must advance"));
    }
    let changed = state
        .connection
        .execute(
            "UPDATE storage_file_sessions SET generation=?1 WHERE id=?2 AND generation IN (?3,?1) AND abandoned=0",
            params![integer(next)?, file.id.as_str(), integer(expected)?],
        )
        .map_err(db)?;
    if changed != 1 {
        return Err(conflict("fence file writer: generation differs or production is abandoned"));
    }
    Ok(())
}

pub(super) fn progress(state: &State, file: &StagedFile) -> Result<FileProgress> {
    state.check()?;
    state.history.check(&file.history)?;
    state
        .connection
        .query_row(
            "SELECT length,finished IS NOT NULL,abandoned FROM storage_files JOIN storage_file_sessions USING(id) WHERE id=?1",
            [file.id.as_str()],
            |row| {
                let finished: bool = row.get(1)?;
                let abandoned: bool = row.get(2)?;
                Ok(FileProgress {
                    length: unsigned(row, 0)?,
                    phase: if finished {
                        FilePhase::Finished
                    } else if abandoned {
                        FilePhase::Abandoned
                    } else {
                        FilePhase::Producing
                    },
                })
            },
        )
        .map_err(db)
}

pub(super) fn abandon(state: &State, file: &StagedFile) -> Result<()> {
    let progress = progress(state, file)?;
    if progress.phase == FilePhase::Finished {
        return Ok(());
    }
    state.connection.execute("UPDATE storage_file_sessions SET abandoned=1,checkpoint=NULL WHERE id=?1", [file.id.as_str()]).map_err(db)?;
    Ok(())
}

pub(super) fn checkpoints(state: &mut State, checkpoints: &[FileCheckpoint]) -> Result<()> {
    state.check()?;
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    for checkpoint in checkpoints {
        state.history.check(&checkpoint.session.history)?;
        checkpoint.sha256()?;
        let (generation, length, prior, abandoned): (u64, u64, Option<Vec<u8>>, bool) = tx
            .query_row(
                "SELECT generation,length,checkpoint,abandoned FROM storage_file_sessions JOIN storage_files USING(id) WHERE id=?1 AND metadata=?2",
                params![checkpoint.session.id.as_str(), encode(&checkpoint.metadata)?],
                |row| Ok((unsigned(row, 0)?, unsigned(row, 1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(db)?;
        if abandoned || generation != checkpoint.session.generation || checkpoint.length > length {
            return Err(conflict("checkpoint file production: generation or length differs"));
        }
        if let Some(prior) = prior {
            let prior: FileCheckpoint = decode(&prior)?;
            if prior.length > checkpoint.length || (prior.length == checkpoint.length && prior.hash_state != checkpoint.hash_state) {
                return Err(conflict("checkpoint file production: durable prefix differs"));
            }
        }
        tx.execute(
            "UPDATE storage_file_sessions SET checkpoint=?1 WHERE id=?2",
            params![encode(checkpoint)?, checkpoint.session.id.as_str()],
        )
        .map_err(db)?;
    }
    tx.commit().map_err(db)
}

pub(super) fn recover(state: &mut State, limits: ReadLimits) -> Result<Vec<FileCheckpoint>> {
    state.check()?;
    let mut output = Vec::new();
    let mut retained = 0;
    {
        let mut query = state.connection.prepare("SELECT checkpoint FROM storage_file_sessions JOIN storage_files USING(id) WHERE checkpoint IS NOT NULL AND abandoned=0 AND finished IS NULL ORDER BY id").map_err(db)?;
        let mut rows = query.query([]).map_err(db)?;
        while let Some(row) = rows.next().map_err(db)? {
            let data: Vec<u8> = row.get(0).map_err(db)?;
            retained += data.len();
            limits.check(output.len() + 1, retained)?;
            let checkpoint: FileCheckpoint = decode(&data)?;
            state.history.check(&checkpoint.session.history)?;
            let file = StagedFile {
                history: checkpoint.session.history.clone(),
                id: checkpoint.session.id.clone(),
            };
            let mut hash = Sha256::new();
            let mut offset = 0;
            while offset < checkpoint.length {
                let maximum =
                    NonZeroUsize::new((checkpoint.length - offset).min(state.settings.chunk_bytes.get() as u64) as usize).unwrap();
                let chunk = bytes(state, &file, offset, maximum, checkpoint.length)?;
                offset += chunk.len() as u64;
                hash.update(chunk);
            }
            if <[u8; 32]>::from(hash.finalize()) != checkpoint.sha256()? {
                return Err(incompatible());
            }
            output.push(checkpoint);
        }
    }
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    for checkpoint in &output {
        tx.execute(
            "DELETE FROM storage_file_chunks WHERE id=?1 AND offset>=?2",
            params![checkpoint.session.id.as_str(), integer(checkpoint.length)?],
        )
        .map_err(db)?;
        tx.execute(
            "UPDATE storage_file_chunks SET data=substr(data,1,?2-offset) WHERE id=?1 AND offset+length(data)>?2",
            params![checkpoint.session.id.as_str(), integer(checkpoint.length)?],
        )
        .map_err(db)?;
        tx.execute("UPDATE storage_files SET length=?2 WHERE id=?1", params![checkpoint.session.id.as_str(), integer(checkpoint.length)?])
            .map_err(db)?;
    }
    tx.commit().map_err(db)?;
    Ok(output)
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileSession, Identifier};

    #[tokio::test]
    async fn resumable_upload_reopens_at_the_checkpoint_and_fences_abandoned_writers() {
        // Persist a Checkpoint and an Uncheckpointed Tail.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("uploads.sqlite");
        let history = StoreHistory {
            store: Identifier::new("store").unwrap(),
            epoch: Identifier::new("epoch").unwrap(),
        };
        let settings = SqliteSettings {
            chunk_rows: NonZeroUsize::new(4).unwrap(),
            chunk_bytes: NonZeroUsize::new(4096).unwrap(),
        };
        let backend = SqliteBackend::new(&path, history.clone(), settings).unwrap();
        backend.create_session().await.unwrap();
        let metadata = FileMetadata {
            id: Identifier::new("upload").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        };
        let file = backend.create_file_production(&metadata).await.unwrap();
        backend.append_file_bytes(&file, 0, FileChunk::new(b"abcdef".to_vec())).await.unwrap();
        let mut hash = Sha256::new();
        hash.update(b"abc");
        let checkpoint = FileCheckpoint::new(
            metadata,
            FileSession {
                history: history.clone(),
                id: file.id.clone(),
                generation: 1,
            },
            3,
            &hash,
        );
        backend.set_file_production_checkpoints(StorageBatch::new(vec![checkpoint.clone()])).await.unwrap();
        backend.delete_session().await.unwrap();
        drop(backend);

        // Recover Only the Durable Prefix and Fence the Old Writer.
        let backend = SqliteBackend::new(&path, history, settings).unwrap();
        backend.create_session().await.unwrap();
        assert_eq!(
            backend
                .list_file_production_checkpoints(ReadLimits {
                    rows: NonZeroUsize::new(4).unwrap(),
                    bytes: NonZeroUsize::new(4096).unwrap()
                })
                .await
                .unwrap(),
            vec![checkpoint]
        );
        assert_eq!(backend.get_file_production(&file).await.unwrap().length, 3);
        assert_eq!(backend.get_file_production_bytes(&file, 0, NonZeroUsize::new(4).unwrap()).await.unwrap(), b"abc");
        backend.set_file_writer_generation(&file, 1, 2).await.unwrap();
        backend.set_file_writer_generation(&file, 1, 2).await.unwrap();
        assert!(backend.set_file_writer_generation(&file, 1, 3).await.is_err());
        backend.append_file_bytes(&file, 3, FileChunk::new(b"XYZ".to_vec())).await.unwrap();

        // Abandon Without Invalidating Retained Content Reads.
        backend.delete_file_production(&file).await.unwrap();
        assert_eq!(backend.get_file_production(&file).await.unwrap().phase, FilePhase::Abandoned);
        assert_eq!(backend.get_file_production_bytes(&file, 0, NonZeroUsize::new(6).unwrap()).await.unwrap(), b"abcXYZ");
        assert!(backend.append_file_bytes(&file, 6, FileChunk::new(b"!".to_vec())).await.is_err());
        assert!(backend.set_file_writer_generation(&file, 2, 3).await.is_err());
        assert!(
            backend
                .list_file_production_checkpoints(ReadLimits {
                    rows: NonZeroUsize::new(4).unwrap(),
                    bytes: NonZeroUsize::new(4096).unwrap()
                })
                .await
                .unwrap()
                .is_empty()
        );
        backend.delete_session().await.unwrap();
    }
}
