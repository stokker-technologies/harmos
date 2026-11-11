/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private WebAssembly Operations Retained Until the Guest Protocol Migration.

/* === Modules === */

pub mod schematic;
pub mod transfer;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use schematic::analyze_schematic_operation;
#[cfg(feature = "runtime")]
pub use schematic::propose_route_operation;
#[cfg(feature = "runtime")]
pub use schematic::validate_schematic_operation;
pub use schematic::{
    ANALYZE_SCHEMATIC_DESCRIPTION, ANALYZE_SCHEMATIC_OPERATION, AnalyzeSchematic, PROPOSE_ROUTE_DESCRIPTION, PROPOSE_ROUTE_OPERATION,
    ProposeRoute, VALIDATE_SCHEMATIC_DESCRIPTION, VALIDATE_SCHEMATIC_OPERATION, ValidateSchematic,
};

/* === Enums === */

static OPERATIONS_CATALOG_DECLARATIONS: [harmos_operations::catalog::Node<harmos_operations::operations::WireDeclaration>; 3] = [
    harmos_operations::catalog::Node::Leaf(VALIDATE_SCHEMATIC_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(PROPOSE_ROUTE_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(ANALYZE_SCHEMATIC_OPERATION.wire()),
];
static OPERATIONS_CATALOG_CALLEES: [harmos_operations::catalog::Node<&'static [harmos_operations::operations::WireDeclaration]>; 3] = [
    harmos_operations::catalog::Node::Leaf(VALIDATE_SCHEMATIC_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(PROPOSE_ROUTE_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(ANALYZE_SCHEMATIC_OPERATION.callees()),
];

pub const OPERATIONS_CATALOG: harmos_operations::execution::operations::Catalog = harmos_operations::execution::operations::Catalog::new(
    concat!(module_path!(), "::Operations"),
    &OPERATIONS_CATALOG_DECLARATIONS,
    &OPERATIONS_CATALOG_CALLEES,
    OPERATIONS_CATALOG_FACTORY,
);

#[cfg(not(feature = "runtime"))]
use declared_catalogs::OPERATIONS_CATALOG_FACTORY;
#[cfg(feature = "runtime")]
use registered_catalogs::OPERATIONS_CATALOG_FACTORY;

#[cfg(feature = "runtime")]
mod registered_catalogs {
    use super::*;
    pub(super) const OPERATIONS_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::RegistrationBatch> =
        Some(operations_catalog_batch);
    fn operations_catalog_batch() -> harmos_operations::execution::__private::RegistrationBatch {
        let mut batch = harmos_operations::execution::__private::RegistrationBatch::new();
        batch = batch.entry(validate_schematic_operation().expect("representable operation"));
        batch = batch.entry(propose_route_operation().expect("representable operation"));
        batch = batch.entry(analyze_schematic_operation().expect("representable operation"));
        batch
    }
}

#[cfg(not(feature = "runtime"))]
mod declared_catalogs {
    pub(super) const OPERATIONS_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::RegistrationBatch> = None;
}
