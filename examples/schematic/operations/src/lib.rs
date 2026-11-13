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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub fn operations_catalog() -> harmos_operations::execution::operations::Catalog {
    operations_catalog_batch().named("schematic.operations")
}

pub fn operations_catalog_batch() -> harmos_operations::execution::__private::Catalog {
    let mut batch = harmos_operations::execution::__private::Catalog::new();
    batch.operations.extend([
        harmos_operations::execution::operations::OperationFactory::protobuf::<ValidateSchematic>().expect("ValidateSchematic declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<ProposeRoute>().expect("ProposeRoute declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeSchematic>().expect("AnalyzeSchematic declaration"),
    ]);
    batch
}

pub fn native_catalog() -> harmos_operations::execution::operations::Catalog {
    native_catalog_batch().named("schematic.native")
}

pub fn native_catalog_batch() -> harmos_operations::execution::__private::Catalog {
    let mut batch = operations_catalog_batch();
    batch.operations.extend([
        harmos_operations::execution::operations::OperationFactory::protobuf::<ListDirectories>().expect("ListDirectories declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeUpload>().expect("AnalyzeUpload declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeExchange>().expect("AnalyzeExchange declaration"),
    ]);
    batch
}

pub fn native_contracts() -> Result<Vec<harmos_operations::operations::Contract>, harmos_operations::operations::ContractError> {
    native_catalog_batch().operations.into_iter().map(|factory| factory.contract()).collect()
}
