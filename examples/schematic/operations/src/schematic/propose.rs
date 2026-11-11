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

#[harmos_operations::operation(
    id = "schematic.route",
    response = Route,
    policy = POLICY,
    visibility = Public,
    replay = harmos_operations::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "ProposeRoute"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProposeRoute {
    pub from: schematic_domain::model::Point,
    pub to: schematic_domain::model::Point,
}

#[harmos_operations::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<schematic_domain::model::Point>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos_operations::Operation for ProposeRoute {
    type Error = crate::Error;

    async fn run(self, _: harmos_operations::Context<'_, Self>) -> Result<harmos_operations::Response<Route>, Self::Error> {
        let corner = schematic_domain::model::Point { x: self.to.x, y: self.from.y };
        let mut points = vec![self.from.clone(), corner, self.to.clone()];
        points.dedup();
        Ok(Route { points }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
