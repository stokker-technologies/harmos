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
use harmos::{Context, Message, Operation, OperationRequest, Plugin, Result, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Message)]
#[harmos(package = "schematic.v1")]
pub struct Snapshot {
    pub symbols: Vec<u64>,
    pub connections: Vec<Connection>,
}

#[derive(Clone, Message)]
#[harmos(package = "schematic.v1")]
pub struct Connection {
    pub from: u64,
    pub to: u64,
}

#[derive(Clone, Debug, Message, PartialEq)]
#[harmos(package = "schematic.v1")]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct ValidateSchematic {
    pub schematic: Snapshot,
}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct Validation {
    pub diagnostics: Vec<String>,
}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct ProposeRoute {
    pub from: Point,
    pub to: Point,
}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct Route {
    pub points: Vec<Point>,
}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct ListDirectories {}

#[derive(Message)]
#[harmos(package = "schematic.v1")]
pub struct Directories {
    pub directories: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationRequest for ValidateSchematic {
    type Response = Validation;
    const ID: &'static str = "schematic.validate";
}

impl OperationRequest for ProposeRoute {
    type Response = Route;
    const ID: &'static str = "schematic.route";
}

impl OperationRequest for ListDirectories {
    type Response = Directories;
    const ID: &'static str = "schematic.list.directories";
}

impl From<schematic_domain::model::Snapshot> for Snapshot {
    fn from(snapshot: schematic_domain::model::Snapshot) -> Self {
        Self {
            symbols: snapshot.symbols,
            connections: snapshot
                .connections
                .into_iter()
                .map(|connection| Connection { from: connection.from, to: connection.to })
                .collect(),
        }
    }
}

/* === Public Functions === */

pub fn plugin(directory: std::path::PathBuf) -> Plugin {
    let validate = Operation::new(ValidateSchematic::ID, validate).visibility(Visibility::Public);
    let route = Operation::new(ProposeRoute::ID, route).visibility(Visibility::Public);
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
    let schematic = schematic_domain::model::Snapshot {
        symbols: request.schematic.symbols,
        connections: request
            .schematic
            .connections
            .into_iter()
            .map(|connection| schematic_domain::model::Connection { from: connection.from, to: connection.to })
            .collect(),
    };
    Ok(Validation { diagnostics: schematic.diagnostics() })
}

async fn route(_: &Context, request: ProposeRoute) -> Result<Route> {
    let corner = Point { x: request.to.x, y: request.from.y };
    let mut points = vec![request.from, corner, request.to];
    points.dedup();
    Ok(Route { points })
}

/* === Tests === */
