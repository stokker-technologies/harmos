/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[harmos::operation(
    id = "schematic.route",
    response = Route,
    policy = crate::POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "ProposeRoute"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProposeRoute {
    pub from: schematic_domain::model::Point,
    pub to: schematic_domain::model::Point,
}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<schematic_domain::model::Point>,
}

#[cfg(feature = "runtime")]
impl harmos::Operation for ProposeRoute {
    type Error = crate::Error;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<Route>, Self::Error> {
        let corner = schematic_domain::model::Point { x: self.to.x, y: self.from.y };
        let mut points = vec![self.from.clone(), corner, self.to.clone()];
        points.dedup();
        Ok(Route { points }.into())
    }
}
