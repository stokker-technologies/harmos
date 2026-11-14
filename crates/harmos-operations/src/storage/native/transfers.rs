/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Unary File Transfer Handlers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::requests as wire;
use harmos_protocol::v1;
use harmos_storage::{StorageCursor, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn range_bytes(encoded: usize, data: usize, limit: usize) -> crate::Result<usize> {
    let overhead = encoded.saturating_sub(limit);
    if overhead > data || data > 0 && overhead == data {
        return Err(harmos_storage::Error::ResourceExhausted {
            resource: "GetFileBytes response metadata",
        }
        .into());
    }
    Ok(data - overhead)
}

pub(super) fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind {
    if let Some(crate::execution::ServingError::Handler(failure)) = error.downcast_ref::<crate::execution::ServingError>() {
        return failure.code();
    }
    error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(crate::operations::FailureKind::Permanent)
}

pub(super) fn receipt(mutation: &wire::MutationId, position: Option<StorageCursor>) -> v1::Receipt {
    let position = position.map(position_to_wire);
    v1::Receipt {
        request_id: format!("{}:{}", mutation.window, mutation.key),
        accepted_at: position.clone(),
        durable_at: position,
        completed: true,
    }
}

pub(super) fn cut(position: StorageCursor) -> v1::ReadCut {
    v1::ReadCut {
        positions: vec![position_to_wire(position)],
        revisions: Vec::new(),
    }
}

pub(super) fn at(cut: Option<v1::ReadCut>) -> crate::Result<Option<StorageCursor>> {
    let Some(cut) = cut else { return Ok(None) };
    if !cut.revisions.is_empty() || cut.positions.len() != 1 {
        return Err(crate::Error::InvalidInput {
            field: "storage at",
            reason: "requires one issued storage position",
        });
    }
    let position = cut.positions.into_iter().next().unwrap();
    let history = required(position.history)?;
    Ok(Some(StorageCursor {
        history: harmos_storage::StoreHistory {
            store: harmos_storage::Identifier::new(history.store).map_err(harmos_domain::Error::from)?,
            epoch: harmos_storage::Identifier::new(history.epoch).map_err(harmos_domain::Error::from)?,
        },
        sequence: position.sequence,
    }))
}

pub(super) fn required<T>(value: Option<T>) -> crate::Result<T> {
    value.ok_or(crate::Error::InvalidInput {
        field: "storage transfer",
        reason: "required value is missing",
    })
}

pub(super) async fn create(
    storage: &StorageManager, metadata: wire::FileMetadata, mutation: wire::MutationId,
) -> crate::Result<(wire::FileProducer, v1::Receipt)> {
    if !storage.capabilities().file_sessions || !storage.capabilities().mutation_receipts {
        return Err(harmos_storage::Error::backend(
            "CreateFileProduction",
            harmos_storage::BackendFailure::Unsupported,
            std::io::Error::new(std::io::ErrorKind::Unsupported, "provider requires resumable sessions and mutation receipts"),
        )
        .into());
    }
    let production = storage.create_file_once(&mutation.clone(), &metadata).await?;
    Ok((production.into(), receipt(&mutation, None)))
}

pub(super) async fn append(
    storage: &StorageManager, session: wire::FileSession, mutation: wire::MutationId, offset: u64, data: Vec<u8>,
) -> crate::Result<(wire::FileProducer, v1::Receipt)> {
    let production = storage.append_file_once(&mutation.clone(), &session, offset, harmos_storage::FileChunk::new(data)).await?;
    Ok((production.into(), receipt(&mutation, None)))
}

pub(super) async fn writer(
    storage: &StorageManager, session: wire::FileSession, mutation: wire::MutationId,
) -> crate::Result<(wire::FileProducer, v1::Receipt)> {
    let production = storage.resume_file_once(&mutation.clone(), &session).await?;
    Ok((production.into(), receipt(&mutation, None)))
}

pub(super) async fn publish(
    storage: &StorageManager, session: wire::FileSession, mutation: wire::MutationId, length: u64, sha256: Vec<u8>,
) -> crate::Result<(wire::FileReference, v1::Receipt)> {
    let digest = sha256.try_into().map_err(|_| crate::Error::InvalidInput {
        field: "PublishFileProduction",
        reason: "SHA-256 requires 32 bytes",
    })?;
    let file = storage.publish_file_production_once(&mutation.clone(), &session, length, digest).await?;
    // Publication's Reference and Receipt Become Durable Together Before Acknowledgment.
    storage.flush_until(&file.position).await?;
    let receipt = receipt(&mutation, Some(file.position.clone()));
    Ok((file.into(), receipt))
}

pub(super) async fn delete(
    storage: &StorageManager, session: wire::FileSession, mutation: wire::MutationId,
) -> crate::Result<(wire::FileProgress, v1::Receipt)> {
    let progress = storage.abandon_file_once(&mutation.clone(), &session).await?;
    Ok((progress.into(), receipt(&mutation, None)))
}

pub(super) async fn get(
    storage: &StorageManager, snapshots: &super::snapshot::SavedSnapshots, file: wire::FileReference, offset: u64, max_bytes: u64,
    at: Option<v1::ReadCut>, scope: Option<&str>,
) -> crate::Result<(harmos_storage::FileBytesRange, v1::ReadCut)> {
    let max_bytes = if max_bytes == 0 { storage.settings().transfer.chunk_bytes() } else { max_bytes };
    let maximum = usize::try_from(max_bytes).ok().and_then(std::num::NonZeroUsize::new).ok_or(crate::Error::InvalidInput {
        field: "GetFileBytes",
        reason: "max_bytes must be positive and representable",
    })?;
    let reference: harmos_storage::FileReference = file.clone();
    if reference.position.history != storage.get_saved_position().history {
        let bytes = super::admission::plan(&file)?;
        let mut selection = ring::digest::Context::new(&ring::digest::SHA256);
        selection.update(b"storage.application_content:");
        selection.update(&(bytes.len() as u64).to_le_bytes());
        selection.update(&bytes);
        selection.update(scope.unwrap_or_default().as_bytes());
        let plan = selection.finish().as_ref().to_vec();
        let (content, read) =
            if let Some(at) = at {
                let read = snapshots
                    .resolve(storage, Some(at))
                    .await
                    .map_err(|error| harmos_storage::Error::backend("GetFileBytes", harmos_storage::BackendFailure::Permanent, error))?;
                let content =
                    read.observation()
                        .filter(|observation| observation.plan == plan)
                        .and_then(|observation| {
                            if let super::file_access::Data::Content(content) = &observation.data { Some(content.clone()) } else { None }
                        })
                        .ok_or(harmos_storage::Error::InvalidInput {
                            field: "GetFileBytes",
                            reason: "content lease does not match reference or caller scope; restart the read",
                        })?;
                (content, read)
            } else {
                let base = snapshots.resolve(storage, None).await?;
                let position = base.at().clone();
                drop(base);
                let content = storage.open_content(&reference, scope, None).await?;
                let resource = plan.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
                let read = snapshots
                    .observe(
                        storage,
                        position,
                        format!("content/{resource}"),
                        super::file_access::Observation {
                            plan,
                            data: super::file_access::Data::Content(content.clone()),
                        },
                    )
                    .await?;
                (content, read)
            };
        let range = content.get_bytes(offset, maximum).await?;
        return Ok((
            harmos_storage::FileBytesRange {
                data: range.data,
                total_length: range.total_length,
                sha256: range.sha256,
                resolved_at: read.at().clone(),
            },
            read.cut().clone(),
        ));
    }
    let read = snapshots.resolve(storage, at).await?;
    let range = read.get_file_bytes(&reference, offset, maximum).await?;
    Ok((range, read.cut().clone()))
}

/* === Private Functions === */

fn position_to_wire(position: StorageCursor) -> v1::Position {
    v1::Position {
        history: Some(v1::History {
            store: position.history.store.to_string(),
            epoch: position.history.epoch.to_string(),
        }),
        sequence: position.sequence,
    }
}

/* === Tests === */
