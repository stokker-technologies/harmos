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
use harmos_storage::StorageManager;

/* === Structs === */

pub struct FollowStream;

/* === Implementations === */

impl ProtobufOperation for FollowStream {
    type Request = harmos_protocol::storage::FollowStream;
    type Response = harmos_protocol::storage::StorageCursor;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::StoredRow>;
    const ID: &'static str = "storage.follow_stream";
    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::StorageCursor> = async {
            let selection: harmos_domain::model::storage::StreamSelection =
                request.selection.ok_or(crate::Error::InvalidInput { field: "selection", reason: "required" })?.try_into()?;
            let after = request.after.map(TryInto::try_into).transpose()?;
            harmos_domain::model::storage::Source::Stream(selection.clone()).validate()?;
            let mut follower = storage.follow::<harmos_storage::StoredRow>(after.as_ref(), selection).await?;
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
