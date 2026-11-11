/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "runtime")]
mod error;
pub mod execution;
pub mod host;
pub mod lifecycle;
pub mod storage;
pub mod streams;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;
pub use harmos_domain::*;
pub use harmos_macros::{catalog, operation};

#[cfg(feature = "lifecycle")]
pub use crate::execution::{Context, Dependent, Executable, ExecutionContext, Operation, Resource, ResourceError, Response};
pub use host::Host;
pub use lifecycle::Lifecycle;
pub use storage::Storage;
pub use streams::Streams;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
extern crate self as harmos_operations;

// Local Imports
#[cfg(feature = "runtime")]
use crate::execution::__private::RegistrationBatch;

/* === Constants === */

#[cfg(feature = "runtime")]
static BUILTIN_DECLARATIONS: [catalog::Node<operations::WireDeclaration>; 4] = [
    catalog::Node::Nested(Host::CATALOG.declarations),
    catalog::Node::Nested(Lifecycle::CATALOG.declarations),
    catalog::Node::Nested(Storage::CATALOG.declarations),
    catalog::Node::Nested(Streams::CATALOG.declarations),
];

#[cfg(feature = "runtime")]
static BUILTIN_CALLEES: [catalog::Node<&'static [operations::WireDeclaration]>; 4] = [
    catalog::Node::Nested(Host::CATALOG.callees),
    catalog::Node::Nested(Lifecycle::CATALOG.callees),
    catalog::Node::Nested(Storage::CATALOG.callees),
    catalog::Node::Nested(Streams::CATALOG.callees),
];

/* === Macros === */

/* === Type Aliases === */

#[cfg(feature = "runtime")]
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

pub struct Builtins;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Builtins {
    pub fn catalog() -> Vec<harmos_domain::OperationDefinition> {
        let mut definitions = Host::catalog();
        definitions.extend(Lifecycle::catalog());
        definitions.extend(Storage::catalog());
        definitions.extend(Streams::catalog());
        definitions
    }
}

#[cfg(feature = "runtime")]
impl crate::execution::operations::OperationCatalog for Builtins {
    const CATALOG: crate::execution::operations::Catalog =
        crate::execution::operations::Catalog::of::<Self>("harmos.builtins", &BUILTIN_DECLARATIONS, &BUILTIN_CALLEES);

    fn batch() -> RegistrationBatch {
        let mut batch = Host::batch();
        batch.operations.extend(Lifecycle::batch().operations);
        batch.operations.extend(Storage::batch().operations);
        batch.operations.extend(Streams::batch().operations);
        batch
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub mod operations {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::operations::*;

    #[cfg(feature = "lifecycle")]
    pub use crate::execution::operations::{Catalog, Executable, OperationCatalog};

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
    pub use crate::execution::__private::RegistrationBatch;

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
        let definitions = crate::Builtins::catalog();
        let ids: std::collections::BTreeSet<_> = definitions.iter().map(|definition| definition.id.as_str()).collect();
        assert_eq!(ids.len(), definitions.len());
        let mut dependencies = definitions.into_iter();
        assert!(dependencies.all(|definition| definition.fingerprint.0 != [0; 32]));
    }
}
