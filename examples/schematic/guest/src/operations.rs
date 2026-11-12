/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Unary Guest Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos::{Context, Operation, OperationRequest, Plugin, Result, Visibility};
use schematic_protocol::{Point, ProposeRoute, Route, ValidateSchematic, Validation};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin() -> Plugin {
    Plugin::new("schematic_tools", env!("CARGO_PKG_VERSION"))
        .operation(Operation::new(ValidateSchematic::ID, validate).visibility(Visibility::Public).policy(super::POLICY))
        .operation(Operation::new(ProposeRoute::ID, route).visibility(Visibility::Public).policy(super::POLICY))
}

/* === Private Functions === */

async fn validate(_: &Context, request: ValidateSchematic) -> Result<Validation> {
    let schematic: schematic_domain::model::Snapshot =
        request.schematic.ok_or_else(|| std::io::Error::other("schematic is required"))?.into();
    Ok(Validation { diagnostics: schematic.diagnostics() })
}
async fn route(_: &Context, request: ProposeRoute) -> Result<Route> {
    let from = request.from.ok_or_else(|| std::io::Error::other("from is required"))?;
    let to = request.to.ok_or_else(|| std::io::Error::other("to is required"))?;
    let corner = Point { x: to.x, y: from.y };
    let mut points = vec![from, corner, to];
    points.dedup();
    Ok(Route { points })
}

/* === Tests === */
