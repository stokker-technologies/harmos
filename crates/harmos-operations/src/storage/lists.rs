/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded Native Lists At Issued Durable Cuts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{
    ReadLimits, StorageManager,
    query::{
        columnar::{Cancellation, ColumnSource},
        protobuf::required,
    },
};
use prost::Message;
use std::num::NonZeroUsize;

/* === Public Functions === */

pub(super) async fn columns(
    context: &ExecutionContext, id: &'static str, selection: Option<wire::ColumnSelection>, at: Option<v1::ReadCut>,
    page: Option<v1::PageRequest>,
) -> Result<(wire::ArrowPage, v1::ReadCut, String), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let selection = required(selection)?;
            let source: ColumnSource = required(selection.source.clone())?.try_into()?;
            super::native::admit(context, id, &source)?;
            let mut plan = selection.clone();
            plan.work = None;
            let plan = [b"storage.columns:".as_slice(), &plan.encode_to_vec()].concat();
            let page = page.unwrap_or_default();
            let (selected, after) = if page.page_token.is_empty() {
                (snapshots.resolve(&storage, at.clone()).await?, None)
            } else {
                let (read, cursor) = snapshots.open_cursor::<harmos_storage::query::columnar::ArrowSeek>(
                    &page.page_token,
                    &plan,
                    storage.settings().response.batch_bytes.get(),
                    at.as_ref(),
                )?;
                (read, Some(cursor))
            };
            let (limits, maximum) = bounds(&storage, &page, selected.cut(), false)?;
            let read = harmos_storage::query::columnar::ArrowScan {
                source: source.clone(),
                columns: selection.columns,
                predicate: selection.predicate.map(|value| harmos_storage::query::columnar::ColumnPredicate {
                    column: value.column,
                    lower: value.lower,
                    upper: value.upper,
                }),
                time: selection.time.map(|value| harmos_storage::query::columnar::TimeBounds {
                    column: value.column,
                    lower_ns: value.lower_ns,
                    upper_ns: value.upper_ns,
                }),
                start: selection.start,
                after,
                limits,
                work: required(selection.work)?.try_into()?,
            };
            read.validate()?;
            let result = selected.list_column_rows(&read, &cancel).await?;
            let next = result.next.as_ref().map(|cursor| snapshots.seal_cursor(&selected, &plan, cursor)).transpose()?.unwrap_or_default();
            let result: wire::ArrowPage = result.try_into()?;
            response_bytes(&result, selected.cut(), &next, maximum)?;
            super::native::admit(context, id, &source)?;
            Ok((result, selected.cut().clone(), next))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn indexes(
    context: &ExecutionContext, id: &'static str, selection: Option<wire::IndexSelection>, source: Option<wire::ColumnSource>,
    at: Option<v1::ReadCut>, page: Option<v1::PageRequest>,
) -> Result<(wire::IndexPage, v1::ReadCut, String), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let selection = required(selection)?;
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let name = harmos_storage::Identifier::new(selection.name.clone()).map_err(harmos_domain::Error::from)?;
            let mut plan = selection.clone();
            plan.work = None;
            let wire_source: wire::ColumnSource = source.clone().into();
            let plan = [b"storage.index:".as_slice(), &plan.encode_to_vec(), &wire_source.encode_to_vec()].concat();
            let page = page.unwrap_or_default();
            let (definition, current) = storage.get_index_definition(&name).await?;
            super::native::source_matches(&source, &definition.source)?;
            let resource = format!("storage.index.{name}");
            let (selected, after) = if page.page_token.is_empty() {
                let read = snapshots.resolve(&storage, at.clone()).await?;
                if revision_at(read.cut(), &resource)?.is_some_and(|expected| expected != current) {
                    return Err(unavailable("ListIndexEntries revision").into());
                }
                let cut = with_revision(read.cut().clone(), &resource, current);
                drop(read);
                let cut = snapshots.composite(&storage, cut).await?;
                (snapshots.resolve(&storage, Some(cut)).await?, None)
            } else {
                let (read, cursor) = snapshots.open_cursor::<harmos_storage::query::index::IndexSeek>(
                    &page.page_token,
                    &plan,
                    storage.settings().response.batch_bytes.get(),
                    at.as_ref(),
                )?;
                (read, Some(cursor))
            };
            let revision = revision_at(selected.cut(), &resource)?.ok_or_else(|| unavailable("ListIndexEntries revision"))?;
            let (limits, maximum) = bounds(&storage, &page, selected.cut(), true)?;
            let read = harmos_storage::query::index::IndexRead {
                revision: Some(revision),
                name,
                lower: selection.lower,
                upper: selection.upper,
                prefix: selection.prefix,
                descending: selection.descending,
                after,
                limits,
                work: required(selection.work)?.try_into()?,
            };
            let result = selected.list_index_entries(&read, &cancel).await?;
            let next = result.next.as_ref().map(|cursor| snapshots.seal_cursor(&selected, &plan, cursor)).transpose()?.unwrap_or_default();
            let result: wire::IndexPage = result.into();
            response_bytes(&result, selected.cut(), &next, maximum)?;
            super::native::admit(context, id, &source)?;
            Ok((result, selected.cut().clone(), next))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn projections(
    context: &ExecutionContext, id: &'static str, selection: Option<wire::ProjectionSelection>, source: Option<wire::ColumnSource>,
    at: Option<v1::ReadCut>, page: Option<v1::PageRequest>,
) -> Result<(wire::ProjectionPage, v1::ReadCut, String), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let selection = required(selection)?;
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let name = harmos_storage::Identifier::new(selection.name.clone()).map_err(harmos_domain::Error::from)?;
            let mut plan = selection.clone();
            plan.work = None;
            let wire_source: wire::ColumnSource = source.clone().into();
            let plan = [b"storage.projection:".as_slice(), &plan.encode_to_vec(), &wire_source.encode_to_vec()].concat();
            let page = page.unwrap_or_default();
            let resource = format!("storage.projection.{name}");
            let requested = if selection.revision.is_empty() { None } else { Some(decode_revision(&selection.revision)?) };
            let (selected, after) = if page.page_token.is_empty() {
                let read = if let Some(at) = at { Some(snapshots.resolve(&storage, Some(at)).await?) } else { None };
                let issued_revision = read.as_ref().map(|read| revision_at(read.cut(), &resource)).transpose()?.flatten();
                if requested.zip(issued_revision).is_some_and(|(requested, issued)| requested != issued) {
                    return Err(unavailable("ListProjectionRows revision").into());
                }
                let (_, revision) = storage.get_projection_definition(&name, issued_revision.or(requested)).await?;
                let frontier = read.as_ref().map(|read| read.at());
                let status = storage
                    .get_projection_status(&name, revision, frontier)
                    .await?
                    .ok_or_else(|| unavailable("ListProjectionRows materialization"))?;
                let mut cut = read.as_ref().map_or_else(|| super::transfers::cut(status.frontier.clone()), |read| read.cut().clone());
                cut = with_revision(cut, &resource, revision);
                drop(read);
                let cut = snapshots.composite(&storage, cut).await?;
                (snapshots.resolve(&storage, Some(cut)).await?, 0usize)
            } else {
                snapshots.open_cursor::<usize>(&page.page_token, &plan, storage.settings().response.batch_bytes.get(), at.as_ref())?
            };
            let revision = revision_at(selected.cut(), &resource)?.ok_or_else(|| unavailable("ListProjectionRows revision"))?;
            if requested.is_some_and(|requested| requested != revision) {
                return Err(unavailable("ListProjectionRows revision").into());
            }
            let (definition, _) = storage.get_projection_definition(&name, Some(revision)).await?;
            super::native::source_matches(&source, &definition.source)?;
            let (limits, maximum) = bounds(&storage, &page, selected.cut(), true)?;
            let read = harmos_storage::query::projection::ProjectionRead {
                name,
                revision,
                frontier: selected.at().clone(),
                after,
                limits,
                work: required(selection.work)?.try_into()?,
            };
            let result = storage.list_projection_rows(&read).await?;
            let next = result.next.as_ref().map(|cursor| snapshots.seal_cursor(&selected, &plan, cursor)).transpose()?.unwrap_or_default();
            let result: wire::ProjectionPage = result.try_into()?;
            response_bytes(&result, selected.cut(), &next, maximum)?;
            super::native::admit(context, id, &source)?;
            Ok((result, selected.cut().clone(), next))
        },
        cancel.clone(),
    )
    .await
}

/* === Private Functions === */

fn bounds(storage: &StorageManager, page: &v1::PageRequest, cut: &v1::ReadCut, repeated_key: bool) -> crate::Result<(ReadLimits, usize)> {
    let settings = storage.settings().response;
    let maximum = if page.max_bytes == 0 {
        settings.batch_bytes.get()
    } else {
        usize::try_from(page.max_bytes).map_err(|_| exhausted())?.min(settings.batch_bytes.get())
    };
    let rows = if page.page_size == 0 {
        settings.batch_rows
    } else {
        NonZeroUsize::new(page.page_size as usize).unwrap().min(settings.batch_rows)
    };
    // Reserve The Cut, Signed Native Cursor And Protobuf Envelope. Index/Projection
    // Keys Can Occur In Both The Payload And Continuation; Keep Their Combined Bound.
    let available = maximum.checked_sub(field(cut.encoded_len()).saturating_add(512)).ok_or_else(exhausted)?;
    let bytes = NonZeroUsize::new(available / if repeated_key { 3 } else { 1 }).ok_or_else(exhausted)?;
    Ok((ReadLimits { rows, bytes }, maximum))
}
fn response_bytes<T: Message>(page: &T, cut: &v1::ReadCut, token: &str, maximum: usize) -> crate::Result<()> {
    let bytes = field(page.encoded_len()) + field(cut.encoded_len()) + if token.is_empty() { 0 } else { field(token.len()) };
    super::native::page_bytes(bytes, maximum)
}
fn field(bytes: usize) -> usize {
    1 + prost::length_delimiter_len(bytes) + bytes
}
pub(super) fn decode_revision(bytes: &[u8]) -> crate::Result<u64> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| unavailable("Storage resource revision"))?;
    let value = u64::from_be_bytes(bytes);
    if value == 0 {
        return Err(unavailable("Storage resource revision").into());
    }
    Ok(value)
}
pub(super) fn revision_at(cut: &v1::ReadCut, resource: &str) -> crate::Result<Option<u64>> {
    cut.revisions.iter().find(|revision| revision.name == resource).map(|revision| decode_revision(&revision.revision)).transpose()
}
pub(super) fn with_revision(mut cut: v1::ReadCut, name: &str, revision: u64) -> v1::ReadCut {
    if !cut.revisions.iter().any(|resource| resource.name == name) {
        cut.revisions.push(v1::ResourceRevision {
            name: name.into(),
            revision: super::definitions::revision(revision),
        });
    }
    cut
}
fn unavailable(domain: &'static str) -> harmos_storage::Error {
    harmos_storage::Error::NotReady { domain }
}
fn exhausted() -> harmos_storage::Error {
    harmos_storage::Error::ResourceExhausted { resource: "Storage list response" }
}
