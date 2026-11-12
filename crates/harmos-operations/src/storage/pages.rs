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

use crate::execution::operations::{ExecutionContext, ServingError, access::GrantAccess};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{
    Query, StorageManager,
    query::{RowSlice, Source, columnar::Cancellation, protobuf::required},
};
use prost::Message;

/* === Structs === */

pub(super) struct Page {
    pub(super) rows: Vec<wire::QueryOutput>,
    pub(super) resolved_at: v1::ReadCut,
    pub(super) next_page_token: String,
}

/* === Public Functions === */

pub(super) fn grant(
    id: &'static str, source: Option<wire::QuerySource>,
) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
    let source: Source = source
        .ok_or_else(|| harmos_domain::MessageError::borrowed("ListRows requires a source"))?
        .try_into()
        .map_err(|error: harmos_domain::Error| harmos_domain::MessageError::new(error.to_string()))?;
    Ok(Some(crate::plugin::Grant::of(id, source.grant_scope())))
}

pub(super) async fn list(
    context: &ExecutionContext, id: &'static str, source: Option<wire::QuerySource>, at: Option<v1::ReadCut>,
    transforms: Vec<wire::Transform>, page: Option<v1::PageRequest>,
) -> Result<Page, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let grants = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let plan = wire::Query {
                source: source.clone(),
                transforms: transforms.clone(),
                at: None,
                slice: None,
            }
            .encode_to_vec();
            let source: Source = required(source)?.try_into()?;
            source.validate()?;
            let grant = crate::plugin::Grant::of(id, source.grant_scope());
            grants.0.admit(&grant).map_err(crate::Error::Access)?;
            let page = page.unwrap_or_default();
            let (read, offset) = if page.page_token.is_empty() {
                (snapshots.resolve(&storage, at).await?, 0)
            } else {
                snapshots.page(&page.page_token, &plan, at.as_ref())?
            };
            let maximum = if page.max_bytes == 0 {
                storage.settings().response.batch_bytes.get()
            } else {
                usize::try_from(page.max_bytes)
                    .map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "ListRows page bytes" })?
                    .min(storage.settings().response.batch_bytes.get())
            };
            let mut count = if page.page_size == 0 {
                storage.settings().response.batch_rows.get()
            } else {
                (page.page_size as usize).min(storage.settings().response.batch_rows.get())
            };
            let transforms = transforms.into_iter().map(TryInto::try_into).collect::<harmos_domain::Result<Vec<_>>>()?;
            loop {
                let query = Query {
                    at: read.at().clone(),
                    source: source.clone(),
                    transforms: transforms.clone(),
                    slice: RowSlice { offset, limit: Some(count as u64) },
                };
                let mut reader = storage.query_snapshot(query, read.snapshot()).await?;
                let mut rows = Vec::new();
                while let Some(value) = reader.next().await? {
                    rows.push(value.output());
                }
                let delivered = reader.completion()?.rows;
                let next_page_token = if delivered == count as u64 {
                    snapshots.token(
                        &read,
                        &plan,
                        offset.checked_add(delivered).ok_or(crate::Error::InvalidInput {
                            field: "ListRows",
                            reason: "page offset overflow",
                        })?,
                    )?
                } else {
                    String::new()
                };
                let resolved_at = read.cut().clone();
                let encoded = rows.iter().map(|row| field(row.encoded_len())).sum::<usize>()
                    + field(resolved_at.encoded_len())
                    + if next_page_token.is_empty() { 0 } else { field(next_page_token.len()) };
                if encoded <= maximum {
                    grants.0.admit(&grant).map_err(crate::Error::Access)?;
                    return Ok(Page { rows, resolved_at, next_page_token });
                }
                if count == 1 {
                    return Err(harmos_storage::Error::ResourceExhausted { resource: "ListRows complete response" }.into());
                }
                count = (count / 2).max(1);
            }
        },
        cancel.clone(),
    )
    .await
}

/* === Private Functions === */

fn field(bytes: usize) -> usize {
    1 + prost::length_delimiter_len(bytes) + bytes
}
