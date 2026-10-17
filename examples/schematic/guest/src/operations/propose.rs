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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Error;
use schematic_domain::operations::ProposeRoute;
use schematic_domain::operations::schematic::propose::Route;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Routing;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_guest::operation]
impl Routing {
    async fn initialize(_: Arc<()>, _: &harmos_guest::Context) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(
        &mut self,
        _context: &harmos_guest::Context,
        request: &mut ProposeRoute,
    ) -> Result<Route, Error> {
        let corner = schematic_domain::model::Point {
            x: request.to.x,
            y: request.from.y,
        };
        let mut points = vec![request.from.clone(), corner, request.to.clone()];
        points.dedup();
        Ok(Route { points })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
