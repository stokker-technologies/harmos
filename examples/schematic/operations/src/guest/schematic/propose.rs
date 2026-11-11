/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

const POLICY: harmos_operations::operations::OperationPolicy = harmos_operations::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_operations::Message)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Route)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProposeRoute {
    pub from: schematic_domain::model::Point,
    pub to: schematic_domain::model::Point,
}

pub static PROPOSE_ROUTE_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<ProposeRoute>("schematic.route", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Idempotent)
        .grpc(harmos_operations::operations::Grpc::new("schematic.v1", "Schematic", "ProposeRoute"));

pub const PROPOSE_ROUTE_OPERATION: harmos_operations::operations::OperationSpec<ProposeRoute> =
    harmos_operations::operations::OperationSpec::<ProposeRoute>::new(&PROPOSE_ROUTE_DESCRIPTION);

#[derive(harmos_operations::Message, Default, Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<schematic_domain::model::Point>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn propose_route_handler(
        request: ProposeRoute, _: harmos_operations::execution::operations::FunctionContext<ProposeRoute>,
    ) -> Result<harmos_operations::Response<Route>, crate::Error> {
        let corner = schematic_domain::model::Point { x: request.to.x, y: request.from.y };
        let mut points = vec![request.from.clone(), corner, request.to.clone()];
        points.dedup();
        Ok(Route { points }.into())
    }

    pub fn propose_route_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            PROPOSE_ROUTE_OPERATION,
            const { &[] },
            propose_route_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::propose_route_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
