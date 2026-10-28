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
pub mod host;
pub mod lifecycle;
pub mod storage;
pub mod streams;
pub mod operations {
    pub use harmos_domain::operations::*;
    #[cfg(feature = "lifecycle")]
    pub use harmos_sdk::operations::{Catalog, Executable, OperationCatalog};
}
#[doc(hidden)]
pub mod __private {
    pub use harmos_domain::__private::*;
    #[cfg(feature = "lifecycle")]
    pub use harmos_sdk::__private::RegistrationBatch;
}

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;
pub use harmos_domain::*;
pub use harmos_macros::{catalog, operation};

#[cfg(feature = "lifecycle")]
pub use harmos_sdk::{Context, Dependent, Executable, ExecutionContext, Operation, Resource, ResourceError, Response};
pub use host::Host;
pub use lifecycle::Lifecycle;
pub use storage::Storage;
pub use streams::Streams;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;

// Local Imports
#[cfg(feature = "runtime")]
use harmos_sdk::__private::RegistrationBatch;

/* === Constants === */

#[cfg(feature = "runtime")]
static BUILTIN_DECLARATIONS: [catalog::Node<operations::WireDeclaration>; 6] = [
    catalog::Node::Nested(Host::CATALOG.declarations),
    catalog::Node::Nested(Lifecycle::CATALOG.declarations),
    catalog::Node::Nested(Storage::CATALOG.declarations),
    catalog::Node::Nested(Streams::CATALOG.declarations),
    catalog::Node::Leaf(operations::WireDeclaration::of::<lifecycle::Initialize>()),
    catalog::Node::Leaf(operations::WireDeclaration::of::<lifecycle::Shutdown>()),
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
        definitions.push(lifecycle::Initialize::definition());
        definitions.push(lifecycle::Shutdown::definition());
        definitions
    }
}

#[cfg(feature = "runtime")]
impl harmos_sdk::operations::OperationCatalog for Builtins {
    const CATALOG: harmos_sdk::operations::Catalog =
        harmos_sdk::operations::Catalog::of::<Self>("harmos.builtins", &BUILTIN_DECLARATIONS, &BUILTIN_CALLEES);

    fn batch() -> RegistrationBatch {
        let mut batch = Host::batch();
        batch.operations.extend(Lifecycle::batch().operations);
        batch.operations.extend(Storage::batch().operations);
        batch.operations.extend(Streams::batch().operations);
        batch.operations.extend(RegistrationBatch::new().operation::<lifecycle::Initialize>().operations);
        batch.operations.extend(RegistrationBatch::new().operation::<lifecycle::Shutdown>().operations);
        batch
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
