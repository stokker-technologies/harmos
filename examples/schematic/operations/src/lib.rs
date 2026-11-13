/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Native Operation Handlers.

/* === Modules === */

pub mod directories;
pub mod schematic;
pub mod transfer;

pub use schematic_protocol as protocol;

mod error;

/* === Re-Exports === */

pub use error::Error;

pub use directories::list_directories::ListDirectories;
pub use schematic::analyze::AnalyzeSchematic;
pub use schematic::exchange::AnalyzeExchange;
pub use schematic::propose::ProposeRoute;
pub use schematic::upload::AnalyzeUpload;
pub use schematic::validate::ValidateSchematic;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_operations::execution::operations::{Catalog, Contract, ContractError, OperationFactory};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn operations_catalog() -> Catalog {
    Catalog::new()
        .named("schematic.operations")
        .entry(OperationFactory::protobuf::<ValidateSchematic>().expect("ValidateSchematic declaration"))
        .entry(OperationFactory::protobuf::<ProposeRoute>().expect("ProposeRoute declaration"))
        .entry(OperationFactory::protobuf::<AnalyzeSchematic>().expect("AnalyzeSchematic declaration"))
}

pub fn native_catalog() -> Catalog {
    operations_catalog()
        .named("schematic.native")
        .entry(OperationFactory::protobuf::<ListDirectories>().expect("ListDirectories declaration"))
        .entry(OperationFactory::protobuf::<AnalyzeUpload>().expect("AnalyzeUpload declaration"))
        .entry(OperationFactory::protobuf::<AnalyzeExchange>().expect("AnalyzeExchange declaration"))
}

pub fn native_contracts() -> Result<Vec<Contract>, ContractError> {
    native_catalog().contracts()
}

/* === Private Functions === */

/* === Tests === */
