/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, ServingError, Visibility};

/* === Constants === */

const POLICY: harmos_operations::execution::operations::OperationPolicy = harmos_operations::execution::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::execution::operations::RetryPolicy::Never,
};

/* === Structs === */

pub struct ProposeRoute;

/* === Implementations === */

impl ProtobufOperation for ProposeRoute {
    type Request = crate::protocol::SchematicProposeRouteRequest;
    type Response = crate::protocol::SchematicProposeRouteResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.route";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const GRPC: Option<harmos_operations::execution::operations::Grpc> =
        Some(harmos_operations::execution::operations::Grpc::new("schematic.host.v1", "Schematic", "ProposeRoute"));
    async fn run(_ctx: ExecutionContext, request: Self::Request, _endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let from = request.from.ok_or(crate::Error::InvalidInput)?;
            let to = request.to.ok_or(crate::Error::InvalidInput)?;
            let corner = crate::protocol::Point { x: to.x, y: from.y };
            let mut points = vec![from, corner, to];
            points.dedup();
            Ok(crate::protocol::SchematicProposeRouteResponse { points })
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> harmos_operations::execution::operations::FailureKind {
        match error.downcast_ref::<crate::Error>() {
            Some(crate::Error::InvalidInput) => harmos_operations::execution::operations::FailureKind::Invalid,
            _ => harmos_operations::execution::operations::FailureKind::Permanent,
        }
    }
}
