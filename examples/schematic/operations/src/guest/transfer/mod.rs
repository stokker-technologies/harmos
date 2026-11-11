/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Transfer Operations for the Host and Guest.

/* === Modules === */

pub mod download;
pub mod hash;
pub mod put;
#[cfg(feature = "runtime")]
pub mod store;
pub mod take;
pub mod upload;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use download::download_operation;
pub use download::{DOWNLOAD_DESCRIPTION, DOWNLOAD_OPERATION, Download};
#[cfg(feature = "runtime")]
pub use hash::hash_operation;
pub use hash::{HASH_DESCRIPTION, HASH_OPERATION, Hash};
#[cfg(feature = "runtime")]
pub use put::put_operation;
pub use put::{PUT_DESCRIPTION, PUT_OPERATION, Put};
#[cfg(feature = "runtime")]
pub use store::Store;
#[cfg(feature = "runtime")]
pub use take::take_operation;
pub use take::{TAKE_DESCRIPTION, TAKE_OPERATION, Take};
#[cfg(feature = "runtime")]
pub use upload::upload_operation;
pub use upload::{UPLOAD_DESCRIPTION, UPLOAD_OPERATION, Upload};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

static TRANSFERS_CATALOG_DECLARATIONS: [harmos_operations::catalog::Node<harmos_operations::operations::WireDeclaration>; 5] = [
    harmos_operations::catalog::Node::Leaf(UPLOAD_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(DOWNLOAD_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(PUT_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(TAKE_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(HASH_OPERATION.wire()),
];
static TRANSFERS_CATALOG_CALLEES: [harmos_operations::catalog::Node<&'static [harmos_operations::operations::WireDeclaration]>; 5] = [
    harmos_operations::catalog::Node::Leaf(UPLOAD_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(DOWNLOAD_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(PUT_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(TAKE_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(HASH_OPERATION.callees()),
];

pub const TRANSFERS_CATALOG: harmos_operations::execution::operations::CatalogDescription =
    harmos_operations::execution::operations::CatalogDescription::new(
        concat!(module_path!(), "::Transfers"),
        &TRANSFERS_CATALOG_DECLARATIONS,
        &TRANSFERS_CATALOG_CALLEES,
        TRANSFERS_CATALOG_FACTORY,
    );

static GUEST_CATALOG_DECLARATIONS: [harmos_operations::catalog::Node<harmos_operations::operations::WireDeclaration>; 2] = [
    harmos_operations::catalog::Node::Nested(crate::guest::OPERATIONS_CATALOG.declarations),
    harmos_operations::catalog::Node::Nested(TRANSFERS_CATALOG.declarations),
];
static GUEST_CATALOG_CALLEES: [harmos_operations::catalog::Node<&'static [harmos_operations::operations::WireDeclaration]>; 2] = [
    harmos_operations::catalog::Node::Nested(crate::guest::OPERATIONS_CATALOG.callees),
    harmos_operations::catalog::Node::Nested(TRANSFERS_CATALOG.callees),
];

pub const GUEST_CATALOG: harmos_operations::execution::operations::CatalogDescription =
    harmos_operations::execution::operations::CatalogDescription::new(
        concat!(module_path!(), "::Guest"),
        &GUEST_CATALOG_DECLARATIONS,
        &GUEST_CATALOG_CALLEES,
        GUEST_CATALOG_FACTORY,
    );

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(not(feature = "runtime"))]
use declared_catalogs::GUEST_CATALOG_FACTORY;
#[cfg(not(feature = "runtime"))]
use declared_catalogs::TRANSFERS_CATALOG_FACTORY;
#[cfg(feature = "runtime")]
use registered_catalogs::GUEST_CATALOG_FACTORY;
#[cfg(feature = "runtime")]
use registered_catalogs::TRANSFERS_CATALOG_FACTORY;

#[cfg(feature = "runtime")]
mod registered_catalogs {
    use super::*;
    pub(super) const TRANSFERS_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::Catalog> =
        Some(transfers_catalog_batch);
    fn transfers_catalog_batch() -> harmos_operations::execution::__private::Catalog {
        let mut batch = harmos_operations::execution::__private::Catalog::new();
        batch = batch.entry(upload_operation().expect("representable operation"));
        batch = batch.entry(download_operation().expect("representable operation"));
        batch = batch.entry(put_operation().expect("representable operation"));
        batch = batch.entry(take_operation().expect("representable operation"));
        batch = batch.entry(hash_operation().expect("representable operation"));
        batch
    }
    pub(super) const GUEST_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::Catalog> = Some(guest_catalog_batch);
    fn guest_catalog_batch() -> harmos_operations::execution::__private::Catalog {
        let mut batch = harmos_operations::execution::__private::Catalog::new();
        batch = batch.extend(crate::guest::OPERATIONS_CATALOG.build().expect("registered guest catalog")).expect("bound nested catalog");
        batch = batch.extend(TRANSFERS_CATALOG.build().expect("registered guest catalog")).expect("bound nested catalog");
        batch
    }
}

#[cfg(not(feature = "runtime"))]
mod declared_catalogs {
    pub(super) const TRANSFERS_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::Catalog> = None;
    pub(super) const GUEST_CATALOG_FACTORY: Option<fn() -> harmos_operations::execution::__private::Catalog> = None;
}
