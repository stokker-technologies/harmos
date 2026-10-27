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

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct ProposeRouteRequest {
    pub from: schematic_domain::model::Point,
    pub to: schematic_domain::model::Point,
}

#[harmos::operation(
    id = "schematic_route",
    request = ProposeRouteRequest, infer_request, response = Route,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "ProposeRoute"),
)]
pub struct ProposeRoute {}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<schematic_domain::model::Point>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for ProposeRoute {
    type Request = ProposeRouteRequest;
    type Response = Route;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(_: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self {})
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let corner = schematic_domain::model::Point { x: request.to.x, y: request.from.y };
        let mut points = vec![request.from.clone(), corner, request.to.clone()];
        points.dedup();
        Ok(Route { points }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
