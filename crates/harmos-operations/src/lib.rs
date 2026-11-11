/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod clock;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod error;
pub mod execution;
pub mod host;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod lifecycle;
pub mod storage;
pub mod streams;

/* === Re-Exports === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use error::Error;
pub use harmos_domain::*;
pub use harmos_macros::Request;

#[cfg(feature = "lifecycle")]
pub use crate::execution::{Context, ExecutionContext, Operation, Resource, ResourceError, Response};
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use host::host_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use lifecycle::lifecycle_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use storage::storage_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use streams::streams_catalog;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
extern crate self as harmos_operations;

// Local Imports
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
use crate::execution::__private::Catalog;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn builtins_catalog() -> crate::execution::operations::Catalog {
    builtins_catalog_batch().named("harmos.builtins")
}

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn builtins_catalog_batch() -> Catalog {
    let mut batch = host_catalog();
    batch.operations.extend(lifecycle_catalog().operations);
    batch.operations.extend(storage_catalog().operations);
    batch.operations.extend(streams_catalog().operations);
    batch
}

/* === Public Functions === */

/* === Private Functions === */

pub mod operations {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::operations::*;

    #[cfg(feature = "lifecycle")]
    pub use crate::execution::operations::{Catalog, CatalogDescription};

    /* === Imports === */

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

    /* === Private Functions === */

    /* === Tests === */
}

#[doc(hidden)]
pub mod __private {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::__private::*;

    #[cfg(feature = "lifecycle")]
    pub use crate::execution::__private::Catalog;

    /* === Imports === */

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

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */

#[cfg(test)]
mod tests {
    #[test]
    fn builtin_inventory() {
        let definitions = crate::builtins_catalog().definitions().unwrap();
        let ids: std::collections::BTreeSet<_> = definitions.iter().map(|definition| definition.id.as_str()).collect();
        assert_eq!(ids.len(), definitions.len());
        let mut dependencies = definitions.into_iter();
        assert!(dependencies.all(|definition| definition.fingerprint.0 != [0; 32]));
    }
}
