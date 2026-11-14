/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Built-In Operations

/* === Modules === */

pub mod access;
pub mod clock;
mod error;
pub mod lifecycle;
pub mod plugins;
pub mod status;

/* === Re-Exports === */

pub use crate::host::host_catalog;
pub use crate::storage::storage_catalog;
pub use crate::streams::streams_catalog;
pub use error::Error;
pub use lifecycle::lifecycle_catalog;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn builtins_catalog() -> crate::execution::operations::Catalog {
    builtins_catalog_batch().named("harmos.builtins")
}
pub fn builtins_catalog_batch() -> crate::execution::__private::Catalog {
    let mut batch = host_catalog();
    batch.operations.extend(lifecycle_catalog().operations);
    batch.operations.extend(storage_catalog().operations);
    batch.operations.extend(streams_catalog().operations);
    batch
}

/* === Private Functions === */

/* === Tests === */
