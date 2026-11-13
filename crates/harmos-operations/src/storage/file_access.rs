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

use crate::execution::operations::{ExecutionContext, ServingError, access::GrantAccess};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{FileSource, StorageManager, query::protobuf::required};
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
    plan: Vec<u8>,
    data: Data,
}

/* === Enums === */

enum Data {
    Production(Production),
    Hash(wire::FileHash),
    Directory(Vec<wire::DirectoryEntry>),
}

/* === Public Functions === */

pub(super) async fn production(
    context: &ExecutionContext, id: &'static str, file: Option<wire::StagedFile>, at: Option<v1::ReadCut>,
) -> Result<(Production, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let file = required(file)?;
        super::transfers::admit(context, id, &file.id)?;
        let mut plan = b"storage.file_production:".to_vec();
        plan.extend(file.encode_to_vec());
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
        let status = storage.get_file_production_status(&file.try_into()?).await?;
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
    result.map_err(super::native::failure)
}

pub(super) async fn hash(
    context: &ExecutionContext, id: &'static str, source: Option<wire::FileSource>, range: Option<wire::ByteRange>, at: Option<v1::ReadCut>,
) -> Result<(wire::FileHash, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let access = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let source = required(source)?;
        let range = required(range)?;
        let mut plan = b"storage.file_hash:".to_vec();
        plan.extend(
            v1::StorageGetFileHashRequest {
                source: Some(source.clone()),
                range: Some(range),
                at: None,
            }
            .encode_to_vec(),
        );
        let source: FileSource = source.try_into()?;
        if let Some(scope) = source.grant_scope() {
            access.0.admit(&crate::plugin::Grant::of(id, scope)).map_err(crate::Error::Access)?;
        }
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
                let hash: wire::FileHash = storage
                    .hash_file(harmos_storage::FileRead {
                        source: FileSource::Local(path),
                        range: range.into(),
                    })
                    .await?
                    .into();
                drop(base);
                let resource = resource("file_hash", &plan);
                let observed =
                    snapshots.observe(&storage, position, resource, Observation { plan, data: Data::Hash(hash.clone()) }).await?;
                Ok((hash, observed.cut().clone()))
            }
        }
    }
    .await;
    result.map_err(super::native::failure)
}

pub(super) async fn capture(
    context: &ExecutionContext, id: &'static str, source: Option<wire::FileSource>, range: Option<wire::ByteRange>,
    metadata: Option<wire::FileMetadata>, consistency: i32,
) -> Result<(wire::FileReference, v1::Receipt), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let access = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let source: FileSource = required(source)?.try_into()?;
        if let Some(scope) = source.grant_scope() {
            access.0.admit(&crate::plugin::Grant::of(id, scope)).map_err(crate::Error::Access)?;
        }
        let metadata: harmos_storage::FileMetadata = required(metadata)?.try_into()?;
        // Capture Reads Its Source And Publishes Its Independently Owned Destination.
        super::transfers::admit(context, id, metadata.id.as_str())?;
        let file = storage
            .capture_file(harmos_storage::FileCapture {
                source,
                range: required(range)?.into(),
                metadata,
                consistency: consistency.try_into()?,
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
    result.map_err(super::native::failure)
}

pub(super) async fn directory(
    context: &ExecutionContext, selection: Option<wire::DirectorySelection>, at: Option<v1::ReadCut>, page: Option<v1::PageRequest>,
) -> Result<v1::StorageListDirectoryEntriesResponse, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let selection = required(selection)?;
        let mut plan = b"storage.directory:".to_vec();
        plan.extend(selection.encode_to_vec());
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
            let mut reader = storage.list_directory(selection.try_into()?).await?;
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
        let mut payload = entries[start..end].iter().map(|entry| field(entry.encoded_len())).sum::<usize>();
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
            payload -= field(entries[end].encoded_len());
        };
        if end == start && start < entries.len() {
            return Err(exhausted().into());
        }
        Ok(v1::StorageListDirectoryEntriesResponse {
            entries: entries[start..end].to_vec(),
            next_page_token: token,
            resolved_at: Some(cut),
        })
    }
    .await;
    result.map_err(super::native::failure)
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
