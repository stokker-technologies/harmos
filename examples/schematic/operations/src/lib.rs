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

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub mod directories;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub mod schematic;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub mod transfer;

pub use schematic_protocol as protocol;

#[cfg(feature = "runtime")]
mod error;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use directories::list_directories::ListDirectories;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use schematic::analyze::AnalyzeSchematic;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use schematic::exchange::AnalyzeExchange;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use schematic::propose::ProposeRoute;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use schematic::upload::AnalyzeUpload;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
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

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub fn operations_catalog() -> harmos_operations::execution::operations::Catalog {
    operations_catalog_batch().named("schematic.operations")
}

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub fn operations_catalog_batch() -> harmos_operations::execution::__private::Catalog {
    let mut batch = harmos_operations::execution::__private::Catalog::new();
    batch.operations.extend([
        harmos_operations::execution::operations::OperationFactory::protobuf::<ValidateSchematic>().expect("ValidateSchematic declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<ProposeRoute>().expect("ProposeRoute declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeSchematic>().expect("AnalyzeSchematic declaration"),
    ]);
    batch
}

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub fn native_catalog() -> harmos_operations::execution::operations::Catalog {
    native_catalog_batch().named("schematic.native")
}

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub fn native_catalog_batch() -> harmos_operations::execution::__private::Catalog {
    let mut batch = operations_catalog_batch();
    batch.operations.extend([
        harmos_operations::execution::operations::OperationFactory::protobuf::<ListDirectories>().expect("ListDirectories declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeUpload>().expect("AnalyzeUpload declaration"),
        harmos_operations::execution::operations::OperationFactory::protobuf::<AnalyzeExchange>().expect("AnalyzeExchange declaration"),
    ]);
    batch
}

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub fn native_contracts() -> Result<Vec<harmos_operations::operations::Contract>, harmos_operations::operations::ContractError> {
    native_catalog_batch().operations.into_iter().map(|factory| factory.contract()).collect()
}
