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
    let mut query = state.connection.prepare("SELECT substr(data,max(0,?2-offset)+1,min(length(data),?3-offset)-max(0,?2-offset)) FROM storage_file_chunks WHERE id=?1 AND offset<?3 AND offset+length(data)>?2 ORDER BY offset").map_err(db)?;
    let chunks =
        query.query_map(params![file.metadata.id.as_str(), integer(offset)?, integer(end)?], |row| row.get::<_, Vec<u8>>(0)).map_err(db)?;
    let mut output = Vec::new();
    for chunk in chunks {
        output.extend(chunk.map_err(db)?);
    }
    if output.len() as u64 != end - offset {
        return Err(incompatible());
    }
    Ok(output)
}

/* === Private Functions === */

/* === Tests === */
