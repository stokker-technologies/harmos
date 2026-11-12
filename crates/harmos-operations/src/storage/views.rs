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
