/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod error;

#[cfg(feature = "grpc")]
pub mod grpc;
pub mod operations;
pub mod plugin;
pub mod services;
pub mod __private {
    pub use crate::operations::response::ErasedResponse;
    pub use harmos_domain::__private::*;
}

/* === Re-Exports === */

pub use harmos_domain::*;
pub use operations::{
    Context, Dependencies, Dependent, EndpointSet, Executable, Handler, InputEndpoint, Instance, Operation, ExecutionContext,
    OutputEndpoint, RegistrationBatch, Resource, ResourceError, Response, ServingError, WireTable,
};
pub use plugin::Plugin;
pub use services::Service;

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
