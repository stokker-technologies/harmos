/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod error;
pub mod lifecycle;
pub mod setup;

#[cfg(feature = "grpc")]
pub mod grpc;
pub mod operations;
pub mod plugin;
pub mod services;
pub mod catalog {
    pub use harmos_domain::catalog::*;
}
#[doc(hidden)]
pub mod __private {
    pub use crate::operations::registration::{PreparedNative, RegistrationBatch, ServiceFuture};
    pub use crate::operations::resources::ResourceSet;
    pub use crate::operations::response::ErasedResponse;
    pub use crate::operations::serving::WireTable;
    pub use harmos_domain::__private::*;
}

pub use harmos_domain::*;
pub use lifecycle::Lifecycle;
pub use operations::{
    Catalog, Context, Dependent, EndpointSet, Executable, ExecutionContext, InputEndpoint, Instance, Operation, OperationCatalog,
    OutputEndpoint, Resource, ResourceError, Response, ServingError, StartupError,
};
pub use plugin::Plugin;
pub use services::Service;
pub use setup::Setup;

#[cfg(feature = "derive")]
pub use harmos_macros::{runtime_catalog as catalog, runtime_operation as operation};

extern crate self as harmos;
