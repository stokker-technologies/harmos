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

use super::requests::{MutationWindow, ProjectionStatus, ReadLimits, SavedSnapshot, StorageCapabilities};

use super::requests::required;

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::v1;
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnSource;
use harmos_storage::StorageManager;

/* === Public Functions === */

pub(super) async fn capabilities(context: &ExecutionContext) -> Result<(StorageCapabilities, Option<MutationWindow>, u64), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let info = storage.info().map_err(|error| super::admission::failure(error.into()))?;
    Ok((storage.capabilities(), info.mutations, info.file_batch_bytes))
}
pub(super) async fn create(context: &ExecutionContext, at: Option<v1::ReadCut>) -> Result<(SavedSnapshot, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    snapshots.create(&storage, at).await.map_err(super::admission::failure)
}
pub(super) async fn delete(context: &ExecutionContext, snapshot: Option<SavedSnapshot>) -> Result<(), ServingError> {
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<()> = async {
        snapshots.release(&required(snapshot)?)?;
        Ok(())
    }
    .await;
    result.map_err(super::admission::failure)
}

pub(super) async fn advance_projection(
    context: &ExecutionContext, snapshot: Option<SavedSnapshot>, name: String, work: Option<ReadLimits>, source: Option<ColumnSource>,
) -> Result<(Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::admission::run(
        context,
        async {
            let source: ColumnSource = required(source)?;

            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, expected) = storage.get_projection_definition(&name, None).await?;
            super::admission::source_matches(&source, &definition.source)?;
            let snapshot = snapshots.read(required(snapshot)?)?;
            let revision = snapshot.advance_projection(&name, Some(expected), required(work)?, &cancel).await?;
            let cut = projection_cut(snapshot.at().clone(), &name, revision);
            Ok((super::definitions::revision(revision), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn projection_status(
    context: &ExecutionContext, name: String, revision: Vec<u8>, at: Option<v1::ReadCut>, source: Option<ColumnSource>,
) -> Result<(Option<ProjectionStatus>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::admission::run(
        context,
        async {
            let source: ColumnSource = required(source)?;

            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let resource = format!("storage.projection.{name}");
            let requested = if revision.is_empty() { None } else { Some(super::lists::decode_revision(&revision)?) };
            let read = if let Some(at) = at { Some(snapshots.resolve(&storage, Some(at)).await?) } else { None };
            let issued = read.as_ref().map(|read| super::lists::revision_at(read.cut(), &resource)).transpose()?.flatten();
            if requested.zip(issued).is_some_and(|(requested, issued)| requested != issued) {
                return Err(harmos_storage::Error::NotReady { domain: "GetProjectionStatus revision" }.into());
            }
            let (definition, revision) = storage.get_projection_definition(&name, issued.or(requested)).await?;
            super::admission::source_matches(&source, &definition.source)?;
            let frontier = read.as_ref().map(|read| read.at());
            let status = storage.get_projection_status(&name, revision, frontier).await?;
            let cut = read.as_ref().map_or_else(
                || super::transfers::cut(status.as_ref().map_or_else(|| storage.get_saved_position(), |status| status.frontier.clone())),
                |read| read.cut().clone(),
            );
            let cut = super::lists::with_revision(cut, &resource, revision);
            drop(read);
            let cut = snapshots.composite(&storage, cut).await?;

            Ok((status, cut))
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
