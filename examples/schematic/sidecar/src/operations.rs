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
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use harmos::{Context, Error, Operation, OperationRequest, Plugin, Result, Visibility};

use harmos_protocol::{plugin::host::SettingsListRequest, v1::setting_value::Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

use schematic_protocol::{Directories, ListDirectories, Point, ProposeRoute, Route, ValidateSchematic, Validation};

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin(directory: PathBuf) -> Plugin {
    let validate = Operation::new(ValidateSchematic::ID, validate).visibility(Visibility::Public);
    let route = Operation::new(ProposeRoute::ID, route).visibility(Visibility::Public);
    let directories = Operation::new(ListDirectories::ID, async move |_: &Context, _: ListDirectories| {
        let entries = std::fs::read_dir(&directory)?;
        let mut directories = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().into_string().map_err(|_| Error::Invalid("List Directories: Name Is Not UTF-8".into()))?;
                directories.push(name);
            }
        }
        directories.sort();
        Ok(Directories { directories })
    })
    .visibility(Visibility::Public);
    Plugin::new("schematic_tools", env!("CARGO_PKG_VERSION"))
        .settings(schematic_protocol::settings_declarations())
        .requires::<SettingsListRequest>()
        .operation(validate)
        .operation(route)
        .operation(directories)
}

/* === Private Functions === */

async fn validate(_: &Context, request: ValidateSchematic) -> Result<Validation> {
    let schematic: schematic_domain::model::Snapshot =
        request.schematic.ok_or_else(|| Error::Invalid("Validate Schematic: Missing Snapshot".into()))?.into();
    Ok(Validation { diagnostics: schematic.diagnostics() })
}

async fn route(context: &Context, request: ProposeRoute) -> Result<Route> {
    let from = request.from.ok_or_else(|| Error::Invalid("Propose Route: Missing From Point".into()))?;
    let to = request.to.ok_or_else(|| Error::Invalid("Propose Route: Missing To Point".into()))?;
    let settings = context.settings().await?.value.ok_or_else(|| Error::Invalid("Route: Missing Settings".into()))?;
    let Some(Value::Flag(horizontal_first)) = settings.values.get("horizontal_first").and_then(|value| value.value.as_ref()) else {
        return Err(Error::Invalid("Route: Missing Horizontal First Flag".into()));
    };
    let corner = if *horizontal_first { Point { x: to.x, y: from.y } } else { Point { x: from.x, y: to.y } };
    let mut points = vec![from, corner, to];
    points.dedup();
    Ok(Route { points })
}

/* === Tests === */
