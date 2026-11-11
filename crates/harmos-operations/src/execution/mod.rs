/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod lifecycle;
pub mod setup;

#[cfg(feature = "grpc")]
pub mod grpc;
pub mod operations;
pub mod services;

/* === Re-Exports === */

pub use harmos_domain::*;
pub use lifecycle::Lifecycle;
pub use operations::{
    Catalog, Context, Dependent, EndpointSet, Executable, ExecutionContext, InputEndpoint, Instance, Invocation, Operation,
    OperationCatalog, OperationHost, OutputEndpoint, Resource, ResourceError, Response, ServingError, StartupError,
};
pub use services::Service;
pub use setup::Setup;

pub use harmos_macros::{runtime_catalog as catalog, runtime_operation as operation};

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

pub mod catalog {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::catalog::*;

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

    pub use crate::execution::operations::registration::{PreparedNative, RegistrationBatch, ServiceFuture};
    pub use crate::execution::operations::resources::ResourceSet;
    pub use crate::execution::operations::response::ErasedResponse;
    pub use harmos_domain::__private::*;

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
