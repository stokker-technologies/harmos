/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared File, Production And Directory Observation

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ServingError, access::GrantAccess};
use harmos_protocol::storage as wire;
use harmos_storage::{FileSource, StorageManager, query::protobuf::required};
use std::{future::Future, num::NonZeroUsize};

/* === Public Functions === */

pub(super) fn grant(
    id: &'static str, source: Option<&wire::FileSource>,
) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
    let source: FileSource = source
        .cloned()
        .ok_or_else(|| harmos_domain::MessageError::borrowed("file source is required"))?
        .try_into()
        .map_err(|error: harmos_domain::Error| harmos_domain::MessageError::new(error.to_string()))?;
    Ok(source.grant_scope().map(|scope| crate::plugin::Grant::of(id, scope)))
}

pub(super) async fn bytes<F, Fut>(
    context: &ExecutionContext, id: &'static str, source: Option<wire::FileSource>, start: Option<wire::FollowStart>, mut emit: F,
) -> Result<wire::FileCompletion, ServingError>
where
    F: FnMut(wire::FileEvent) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let access = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
    let work: Result<_, crate::Error> = async {
        let source: FileSource = required(source)?.try_into()?;
        let grant = source.grant_scope().map(|scope| crate::plugin::Grant::of(id, scope));
        if let Some(grant) = &grant {
            access.0.admit(grant).map_err(crate::Error::Access)?;
        }
        if let FileSource::Stored(file) = &source {
            if !storage.get_saved_position().covers(&file.position)? {
                return Err(harmos_storage::Error::NotReady { domain: "WatchFileBytes durable content" }.into());
            }
        }
        Ok((
            storage
                .follow_file(harmos_storage::FileFollow {
                    source,
                    start: required(start)?.try_into()?,
                })
                .await?,
            grant,
        ))
    }
    .await;
    let (mut reader, grant) = work.map_err(super::native::failure)?;
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next() => next.map_err(|error| super::native::failure(error.into()))?,
        };
        let Some(event) = next else {
            return reader.completion().map(Into::into).map_err(|error| super::native::failure(error.into()));
        };
        if let Some(grant) = &grant {
            access.0.admit(grant).map_err(|error| super::native::failure(crate::Error::Access(error)))?;
        }
        let event: wire::FileEvent = (*event).clone().into();
        match event.event {
            Some(wire::file_event::Event::Data(batch)) => {
                let mut start = 0;
                while start < batch.bytes.len() {
                    let offset = batch.offset + start as u64;
                    let count = capacity(storage.settings().response.batch_bytes.get(), offset, batch.bytes.len() - start, 2)
                        .map_err(|error| super::native::failure(error.into()))?;
                    emit(wire::FileEvent {
                        event: Some(wire::file_event::Event::Data(wire::FileBatch {
                            offset,
                            bytes: batch.bytes[start..start + count].to_vec(),
                        })),
                    })
                    .await?;
                    start += count;
                }
            }
            event => emit(wire::FileEvent { event }).await?,
        }
    }
}

pub(super) async fn production<F, Fut>(
    context: &ExecutionContext, id: &'static str, file: Option<wire::StagedFile>, offset: u64, mut emit: F,
) -> Result<wire::FileProgress, ServingError>
where
    F: FnMut(wire::FileBatch) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let file: harmos_storage::StagedFile = required(file)?.try_into()?;
        super::transfers::admit(context, id, file.id.as_str())?;
        Ok((storage.follow_production(&file, offset).await?, file.id))
    }
    .await;
    let (mut reader, file) = result.map_err(super::native::failure)?;
    let mut next_offset = offset;
    loop {
        let maximum = NonZeroUsize::new(
            capacity(storage.settings().response.batch_bytes.get(), next_offset, storage.settings().response.batch_bytes.get(), 1)
                .map_err(|error| super::native::failure(error.into()))?,
        )
        .unwrap();
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next_batch(maximum) => next.map_err(|error| super::native::failure(error.into()))?,
        };
        let Some(batch) = next else {
            return Ok(reader.progress().into());
        };
        super::transfers::admit(context, id, file.as_str()).map_err(super::native::failure)?;
        next_offset = batch.offset() + batch.bytes().len() as u64;
        emit(wire::FileBatch {
            offset: batch.offset(),
            bytes: batch.bytes().to_vec(),
        })
        .await?;
    }
}

pub(super) async fn directory<F, Fut>(
    context: &ExecutionContext, selection: Option<wire::DirectorySelection>, include_existing: bool, mut emit: F,
) -> Result<(), ServingError>
where
    F: FnMut(wire::DirectoryEvent) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async { Ok(storage.watch_directory(required(selection)?.try_into()?, include_existing).await?) }.await;
    let mut reader = result.map_err(super::native::failure)?;
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next() => next.map_err(|error| super::native::failure(error.into()))?,
        };
        let Some(event) = next else {
            return Ok(());
        };
        emit((*event).clone().into()).await?;
    }
}

pub(super) async fn send<T: prost_reflect::ReflectMessage + Default + Send + Sync + 'static>(
    context: &ExecutionContext, id: &'static str, output: &crate::execution::operations::ProtobufOutput<T>, response: T,
) -> Result<(), ServingError> {
    let response = super::native::read_response(context, response)?;
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    storage
        .clock()
        .timeout(super::IO_TIMEOUT, output.send(response))
        .await
        .map_err(|_| {
            super::native::failure(
                harmos_storage::Error::backend(
                    id,
                    harmos_storage::BackendFailure::Transient,
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "consumer delivery deadline exceeded"),
                )
                .into(),
            )
        })?
        .map_err(ServingError::Stream)
}

/* === Private Functions === */

// Count Full Nested Protobuf Frames Without Allocating Candidate Payloads.
fn capacity(ceiling: usize, offset: u64, remaining: usize, wrappers: usize) -> harmos_storage::Result<usize> {
    let mut lower = 0;
    let mut upper = remaining.min(ceiling);
    while lower < upper {
        let count = lower + (upper - lower).div_ceil(2);
        let mut length = if offset == 0 { 0 } else { 1 + prost::encoding::encoded_len_varint(offset) };
        length += 1 + prost::length_delimiter_len(count) + count;
        for _ in 0..wrappers {
            length = 1 + prost::length_delimiter_len(length) + length;
        }
        if length <= ceiling {
            lower = count;
        } else {
            upper = count - 1;
        }
    }
    if lower == 0 {
        return Err(harmos_storage::Error::ResourceExhausted {
            resource: "file observation response framing",
        });
    }
    Ok(lower)
}
