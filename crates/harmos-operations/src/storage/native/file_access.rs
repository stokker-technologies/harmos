/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Unary Content And Filesystem Metadata Reads

/* === Imports === */

use super::requests::required;

use super::requests as wire;
use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::v1;
use harmos_storage::{FileSource, StorageManager};
use prost::Message;
use ring::digest;

/* === Structs === */

#[derive(Clone)]
pub(super) struct Production {
    pub(super) progress: wire::FileProgress,
    pub(super) generation: u64,
    pub(super) accepted: u64,
    pub(super) durable: u64,
}

pub(super) struct Observation {
    pub(super) plan: Vec<u8>,
    pub(super) data: Data,
}

/* === Enums === */

pub(super) enum Data {
    Content(harmos_storage::ContentRead),
    Production(Production),
    Hash(wire::FileHash),
    Directory(Vec<wire::DirectoryEntry>),
}

/* === Public Functions === */

pub(super) async fn production(
    context: &ExecutionContext, _id: &'static str, file: Option<wire::StagedFile>, at: Option<v1::ReadCut>,
) -> Result<(Production, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let file = required(file)?;

        let mut plan = b"storage.file_production:".to_vec();
        plan.extend(super::admission::plan(&file)?);
        if let Some(at) = at {
            let read = snapshots.resolve(&storage, Some(at)).await?;
            let observation = read.observation().filter(|observation| observation.plan == plan).ok_or_else(expired)?;
            let Data::Production(production) = &observation.data else {
                return Err(expired().into());
            };
            return Ok((production.clone(), read.cut().clone()));
        }
        let base = snapshots.resolve(&storage, None).await?;
        let position = base.at().clone();
        let status = storage.get_file_production_status(&file).await?;
        let production = Production {
            progress: status.progress.into(),
            generation: status.writer_generation.unwrap_or(0),
            accepted: status.accepted_length,
            durable: status.durable_length,
        };
        drop(base);
        let observed = snapshots
            .observe(
                &storage,
                position,
                resource("file_production", &plan),
                Observation {
                    plan,
                    data: Data::Production(production.clone()),
                },
            )
            .await?;
        Ok((production, observed.cut().clone()))
    }
    .await;
    result.map_err(super::admission::failure)
}

pub(super) async fn hash(
    context: &ExecutionContext, _id: &'static str, source: Option<wire::FileSource>, range: Option<wire::ByteRange>,
    at: Option<v1::ReadCut>,
) -> Result<(wire::FileHash, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;

    let result: crate::Result<_> = async {
        let source = required(source)?;
        let range = required(range)?;
        let mut plan = b"storage.file_hash:".to_vec();
        plan.extend(super::admission::plan(&(source.clone(), range))?);
        let source: FileSource = source;
        match source {
            FileSource::Stored(file) => {
                let read = snapshots.resolve(&storage, at).await?;
                Ok((read.hash_file(&file, range.into()).await?.into(), read.cut().clone()))
            }
            FileSource::Local(path) => {
                if let Some(at) = at {
                    let read = snapshots.resolve(&storage, Some(at)).await?;
                    let observation = read.observation().filter(|observation| observation.plan == plan).ok_or_else(expired)?;
                    let Data::Hash(hash) = &observation.data else {
                        return Err(expired().into());
                    };
                    return Ok((hash.clone(), read.cut().clone()));
                }
                // Select The Primary Boundary Before Reading External Metadata.
                let base = snapshots.resolve(&storage, None).await?;
                let position = base.at().clone();
                let hash: wire::FileHash =
                    storage.hash_file(harmos_storage::FileRead { source: FileSource::Local(path), range }).await?.into();
                drop(base);
                let resource = resource("file_hash", &plan);
                let observed =
                    snapshots.observe(&storage, position, resource, Observation { plan, data: Data::Hash(hash.clone()) }).await?;
                Ok((hash, observed.cut().clone()))
            }
        }
    }
    .await;
    result.map_err(super::admission::failure)
}

pub(super) async fn capture(
    context: &ExecutionContext, _id: &'static str, source: Option<wire::FileSource>, range: Option<wire::ByteRange>,
    metadata: Option<wire::FileMetadata>, consistency: wire::CaptureConsistency,
) -> Result<(wire::FileReference, v1::Receipt), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;

    let result: crate::Result<_> = async {
        let source: FileSource = required(source)?;
        let metadata: harmos_storage::FileMetadata = required(metadata)?;
        // Capture Reads Its Source And Publishes Its Independently Owned Destination.

        let file = storage
            .capture_file(harmos_storage::FileCapture {
                source,
                range: required(range)?.into(),
                metadata,
                consistency,
            })
            .await?;
        let cut = snapshots.composite(&storage, super::transfers::cut(file.position.clone())).await?;
        let position = cut.positions.into_iter().next().expect("one published position");
        let receipt = v1::Receipt {
            request_id: format!("capture:{}:{}:{}", file.position.history.store, file.position.history.epoch, file.position.sequence),
            accepted_at: Some(position.clone()),
            durable_at: Some(position),
            completed: true,
        };
        Ok((file.into(), receipt))
    }
    .await;
    result.map_err(super::admission::failure)
}

pub(super) async fn directory(
    context: &ExecutionContext, selection: Option<wire::DirectorySelection>, at: Option<v1::ReadCut>, page: Option<v1::PageRequest>,
) -> Result<wire::ListDirectoryEntriesResponse, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let selection = required(selection)?;
        let mut plan = b"storage.directory:".to_vec();
        plan.extend(super::admission::plan(&selection)?);
        let page = page.unwrap_or_default();
        let maximum = if page.max_bytes == 0 {
            storage.settings().response.batch_bytes.get()
        } else {
            usize::try_from(page.max_bytes).map_err(|_| exhausted())?.min(storage.settings().response.batch_bytes.get())
        };
        let rows = if page.page_size == 0 {
            storage.settings().response.batch_rows.get()
        } else {
            (page.page_size as usize).min(storage.settings().response.batch_rows.get())
        };
        let (read, start) = if !page.page_token.is_empty() {
            snapshots.open_cursor::<usize>(&page.page_token, &plan, storage.settings().response.batch_bytes.get(), at.as_ref())?
        } else if let Some(at) = at {
            (snapshots.resolve(&storage, Some(at)).await?, 0)
        } else {
            let base = snapshots.resolve(&storage, None).await?;
            let position = base.at().clone();
            let mut reader = storage.list_directory(selection).await?;
            let mut entries = Vec::new();
            while let Some(entry) = reader.next().await? {
                entries.push((*entry).clone());
            }
            reader.completion()?;
            entries.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            if entries.windows(2).any(|pair| pair[0].path == pair[1].path) {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "ListDirectoryEntries",
                    reason: "directory provider repeated a path",
                }
                .into());
            }
            drop(base);
            let resource = resource("directory", &plan);
            let read = snapshots
                .observe(
                    &storage,
                    position,
                    resource,
                    Observation {
                        plan: plan.clone(),
                        data: Data::Directory(entries),
                    },
                )
                .await?;
            (read, 0)
        };
        let observation = read.observation().filter(|observation| observation.plan == plan).ok_or_else(expired)?;
        let Data::Directory(entries) = &observation.data else {
            return Err(expired().into());
        };
        if start > entries.len() {
            return Err(expired().into());
        }
        let candidate = start.saturating_add(rows).min(entries.len());
        let mut end = candidate;
        let mut payload = entries[start..end].iter().map(|entry| field(entry.memory_size())).sum::<usize>();
        let cut = read.cut().clone();
        let token = loop {
            let token = if end < entries.len() { snapshots.seal_cursor(&read, &plan, &end)? } else { String::new() };
            let bytes = payload + field(cut.encoded_len()) + if token.is_empty() { 0 } else { field(token.len()) };
            if bytes <= maximum {
                break token;
            }
            if end == start {
                return Err(exhausted().into());
            }
            end -= 1;
            payload -= field(entries[end].memory_size());
        };
        if end == start && start < entries.len() {
            return Err(exhausted().into());
        }
        Ok(wire::ListDirectoryEntriesResponse {
            entries: entries[start..end].to_vec(),
            next_page_token: token,
            resolved_at: Some(cut),
        })
    }
    .await;
    result.map_err(super::admission::failure)
}

/* === Private Functions === */

fn resource(kind: &str, plan: &[u8]) -> String {
    let digest = digest::digest(&digest::SHA256, plan);
    format!("storage.observation.{kind}.{}", digest.as_ref().iter().map(|byte| format!("{byte:02x}")).collect::<String>())
}
fn field(bytes: usize) -> usize {
    1 + prost::length_delimiter_len(bytes) + bytes
}
fn exhausted() -> harmos_storage::Error {
    harmos_storage::Error::ResourceExhausted {
        resource: "ListDirectoryEntries complete page",
    }
}
fn expired() -> harmos_storage::Error {
    harmos_storage::Error::InvalidInput {
        field: "storage continuation",
        reason: "external metadata cut expired or its selection changed; restart the read",
    }
}
