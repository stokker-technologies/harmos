/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct FollowRecords;

/* === Implementations === */

impl ProtobufOperation for FollowRecords {
    type Request = harmos_protocol::storage::FollowRecords;
    type Response = harmos_protocol::storage::StorageCursor;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::Followed>;
    const ID: &'static str = "storage.follow_records";
    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "FollowRecords"));
    const VISIBILITY: Visibility = Visibility::Public;
    fn grant(request: &Self::Request) -> Result<Option<harmos_operations::plugin::Grant>, harmos_domain::MessageError> {
        let schema = request
            .selection
            .as_ref()
            .and_then(|selection| selection.schema.as_ref())
            .ok_or_else(|| harmos_domain::MessageError::borrowed("selection schema is required"))?;
        Ok(Some(harmos_operations::plugin::Grant::of(Self::ID, schema.id.clone())))
    }

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::StorageCursor> = async {
            let selection: harmos_domain::model::storage::RecordSelection =
                request.selection.ok_or(crate::Error::InvalidInput { field: "selection", reason: "required" })?.try_into()?;
            let after = request.after.map(TryInto::try_into).transpose()?;
            harmos_domain::model::storage::Source::Records(selection.clone()).validate()?;
            let mut follower = storage.follow::<harmos_storage::StoredRecord>(after.as_ref(), selection).await?;
            output
                .send(harmos_protocol::storage::Followed {
                    followed: Some(harmos_protocol::storage::followed::Followed::Boundaries(harmos_protocol::storage::FollowBoundaries {
                        saved: Some(follower.saved_boundary().clone().into()),
                        accepted: Some(follower.accepted_boundary().clone().into()),
                    })),
                })
                .await?;
            loop {
                let page = follower.next().await?;
                for entry in page.entries() {
                    output
                        .send(harmos_protocol::storage::Followed {
                            followed: Some(harmos_protocol::storage::followed::Followed::Record(entry.clone().into())),
                        })
                        .await?;
                }
            }
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
}
