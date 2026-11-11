/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::access::GrantAccess;
use crate::execution::operations::{
    ExecutionContext, Grpc, ProtobufInput, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility,
};
use harmos_operations::operations::FailureKind;
use harmos_storage::StorageManager;

/* === Structs === */

pub struct WriteObjects;

/* === Implementations === */

impl ProtobufOperation for WriteObjects {
    type Request = harmos_protocol::storage::WriteObjects;
    type Response = harmos_protocol::storage::WriteCompletion;
    type Endpoints = (ProtobufInput<harmos_protocol::storage::DataBatch>, ProtobufOutput<harmos_protocol::storage::WriteReceipt>);
    const ID: &'static str = "storage.write_objects";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "WriteObjects"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    async fn run(context: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let grants = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_protocol::storage::WriteCompletion> = async {
            let scope = request.scope.map(harmos_domain::Identifier::new).transpose().map_err(harmos_domain::Error::from)?;
            super::exchange::exchange(&storage, &grants.0, scope.as_ref(), endpoints, super::exchange::WriteKind::Objects, Self::ID).await
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
