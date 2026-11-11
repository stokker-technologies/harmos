/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Unary Contracts Shared by the Native Plugin and Its Host.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::{Context, Operation, OperationRequest, Plugin, Result, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

use schematic_protocol::{Directories, ListDirectories, Point, ProposeRoute, Route, ValidateSchematic, Validation};

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin(directory: std::path::PathBuf) -> Plugin {
    let validate = Operation::new(VALIDATE_SCHEMATIC_OPERATION.entry().id, validate).visibility(Visibility::Public);
    let route = Operation::new(PROPOSE_ROUTE_OPERATION.entry().id, route).visibility(Visibility::Public);
    let directories = Operation::new(ListDirectories::ID, async move |_: &Context, _: ListDirectories| {
        let entries = std::fs::read_dir(&directory)?;
        let mut directories = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().into_string().map_err(|_| std::io::Error::other("Directory Name Is Not UTF-8"))?;
                directories.push(name);
            }
        }
        directories.sort();
        Ok(Directories { directories })
    })
    .visibility(Visibility::Public);
    Plugin::new("schematic_tools", env!("CARGO_PKG_VERSION")).operation(validate).operation(route).operation(directories)
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
