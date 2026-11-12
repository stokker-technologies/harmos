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

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{
    StorageManager,
    query::{
        columnar::{Cancellation, ColumnSource},
        protobuf::required,
    },
};

/* === Public Functions === */

pub(super) async fn capabilities(context: &ExecutionContext) -> Result<wire::StorageCapabilities, ServingError> {
    Ok(context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?.capabilities().into())
}
pub(super) async fn create(
    context: &ExecutionContext, at: Option<v1::ReadCut>,
) -> Result<(wire::SavedSnapshot, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    snapshots.create(&storage, at).await.map_err(super::native::failure)
}
pub(super) async fn delete(context: &ExecutionContext, snapshot: Option<wire::SavedSnapshot>) -> Result<(), ServingError> {
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<()> = async {
        snapshots.release(&required(snapshot)?)?;
        Ok(())
    }
    .await;
    result.map_err(super::native::failure)
}
pub(super) async fn columns(
    context: &ExecutionContext, id: &'static str, snapshot: Option<wire::SavedSnapshot>, read: Option<wire::ArrowScan>,
) -> Result<(wire::ArrowPage, v1::ReadCut), ServingError> {
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let read: harmos_storage::query::columnar::ArrowScan = required(read)?.try_into()?;
            super::native::admit(context, id, &read.source)?;
            let snapshot = snapshots.read(required(snapshot)?)?;
            let page = snapshot.list_column_rows(&read, &cancel).await?;
            super::native::admit(context, id, &read.source)?;
            let page: wire::ArrowPage = page.try_into()?;
            super::native::page_bytes(page.buffer_bytes(), read.limits.bytes.get())?;
            Ok((page, snapshot.cut().clone()))
        },
        cancel.clone(),
    )
    .await
}
pub(super) async fn indexes(
    context: &ExecutionContext, id: &'static str, snapshot: Option<wire::SavedSnapshot>, read: Option<wire::IndexRead>,
    source: Option<wire::ColumnSource>,
) -> Result<(wire::IndexPage, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            let read: harmos_storage::query::index::IndexRead = required(read)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let (definition, _) = storage.get_index_definition(&read.name).await?;
            super::native::source_matches(&source, &definition.source)?;
            let snapshot = snapshots.read(required(snapshot)?)?;
            let page: wire::IndexPage = snapshot.list_index_entries(&read, &cancel).await?.into();
            super::native::admit(context, id, &source)?;
            super::native::page_bytes(prost::Message::encoded_len(&page), read.limits.bytes.get())?;
            Ok((page, snapshot.cut().clone()))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn advance_projection(
    context: &ExecutionContext, id: &'static str, snapshot: Option<wire::SavedSnapshot>, name: String, work: Option<wire::ReadLimits>,
    source: Option<wire::ColumnSource>,
) -> Result<(Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, expected) = storage.get_projection_definition(&name, None).await?;
            super::native::source_matches(&source, &definition.source)?;
            let snapshot = snapshots.read(required(snapshot)?)?;
            let revision = snapshot.advance_projection(&name, Some(expected), required(work)?.try_into()?, &cancel).await?;
            let cut = projection_cut(snapshot.at().clone(), &name, revision);
            Ok((super::definitions::revision(revision), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn projection_status(
    context: &ExecutionContext, id: &'static str, name: String, revision: u64, frontier: Option<wire::StorageCursor>,
    source: Option<wire::ColumnSource>,
) -> Result<(Option<wire::ProjectionStatus>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, _) = storage.get_projection_definition(&name, Some(revision)).await?;
            super::native::source_matches(&source, &definition.source)?;
            let frontier: Option<harmos_storage::StorageCursor> = frontier.map(TryInto::try_into).transpose()?;
            let retained = if let Some(frontier) = &frontier {
                Some(snapshots.resolve(&storage, Some(projection_cut(frontier.clone(), &name, revision))).await?)
            } else {
                None
            };
            let status = storage.get_projection_status(&name, revision, frontier.as_ref()).await?;
            let cut = projection_cut(
                status.as_ref().map_or_else(|| frontier.unwrap_or_else(|| storage.get_saved_position()), |status| status.frontier.clone()),
                &name,
                revision,
            );
            let cut = if let Some(read) = retained { read.cut().clone() } else { snapshots.composite(&storage, cut).await? };
            super::native::admit(context, id, &source)?;
            Ok((status.map(Into::into), cut))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn projection_rows(
    context: &ExecutionContext, id: &'static str, read: Option<wire::ProjectionRead>, source: Option<wire::ColumnSource>,
) -> Result<(wire::ProjectionPage, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let mut read = required(read)?;
            let name = harmos_storage::Identifier::new(read.name.clone()).map_err(harmos_domain::Error::from)?;
            let (definition, _) = storage.get_projection_definition(&name, Some(read.revision)).await?;
            super::native::source_matches(&source, &definition.source)?;
            let selected = if let Some(frontier) = &read.frontier {
                snapshots.resolve(&storage, Some(projection_cut(frontier.clone().try_into()?, &name, read.revision))).await?
            } else {
                let status = storage.get_projection_status(&name, read.revision, None).await?.ok_or(harmos_storage::Error::NotReady {
                    domain: "ListProjectionRows materialization",
                })?;
                let cut = snapshots.composite(&storage, projection_cut(status.frontier.clone(), &name, read.revision)).await?;
                read.frontier = Some(status.frontier.into());
                snapshots.resolve(&storage, Some(cut)).await?
            };
            let read: harmos_storage::query::projection::ProjectionRead = read.try_into()?;
            let page: wire::ProjectionPage = storage.list_projection_rows(&read).await?.try_into()?;
            super::native::page_bytes(page.buffer_bytes(), read.limits.bytes.get())?;
            super::native::admit(context, id, &source)?;
            Ok((page, selected.cut().clone()))
        },
        cancel.clone(),
    )
    .await
}

fn projection_cut(position: harmos_storage::StorageCursor, name: &harmos_storage::Identifier, revision: u64) -> v1::ReadCut {
    let mut cut = super::transfers::cut(position);
    cut.revisions.push(v1::ResourceRevision {
        name: format!("storage.projection.{name}"),
        revision: super::definitions::revision(revision),
    });
    cut
}
