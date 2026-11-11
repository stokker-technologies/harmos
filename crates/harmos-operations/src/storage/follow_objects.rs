/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ProtobufOperation, ProtobufOutput, Resource, ServingError};
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Structs === */

pub struct FollowObjects;

/* === Implementations === */

impl ProtobufOperation for FollowObjects {
    type Request = harmos_protocol::storage::FollowObjects;
    type Response = harmos_protocol::storage::StorageCursor;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::ObjectEntry>;
    const ID: &'static str = "storage.follow_objects";
    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager<ErasedCatalog>>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager<ErasedCatalog>>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::StorageCursor> = async {
            let selection: harmos_domain::model::storage::ObjectSelection =
                request.selection.ok_or(crate::Error::InvalidInput { field: "selection", reason: "required" })?.try_into()?;
            let after = request.after.map(TryInto::try_into).transpose()?;
            if !selection.follows_revisions() {
                return Err(crate::Error::InvalidInput {
                    field: "storage follow",
                    reason: "following delivers every object revision",
                });
            }
            let mut follower = storage.follow::<harmos_storage::ObjectEntry>(after.as_ref(), selection).await?;
            loop {
                let page = follower.next().await?;
                for entry in page.entries() {
                    output.send(entry.clone().into()).await?;
                }
            }
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
}
