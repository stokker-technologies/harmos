/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod bundle;
pub mod clock;
pub mod config;
mod error;
pub mod filesystem;

mod application;
#[cfg(feature = "grpc")]
pub mod grpc;
pub mod prelude;
pub mod records;
pub mod state;
pub mod storage;

#[cfg(not(target_arch = "wasm32"))]
pub mod plugin;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin-guest"))]
pub mod guest;
pub mod operations;
pub mod runtime;

pub mod services;
#[cfg(feature = "simulation")]
pub mod simulation;
pub mod streams;
mod supervision;

/* === Re-Exports === */

pub use application::{Application, Startup};
pub use error::{BackendFailure, Error};
pub use harmos_domain as domain;
pub use harmos_domain::{DefinitionId, Identified};
pub use harmos_operations as builtins;
pub use harmos_operations::model;

pub use harmos_domain::{
    Aggregate, Identifier, IdentifierError, MemorySize, Message, MessageError, Metadata, RowMetadata, Storable, StreamField, Value,
    ValueError, btree_map_memory,
};
pub use harmos_domain::{Digest, Fingerprint, Portable, QualifiedId};
pub use harmos_operations::execution::operations::{EndpointSet, OperationHost, Response};
pub use harmos_operations::execution::{Context, ExecutionContext, Lifecycle, Operation, Resource, ResourceError, Service, Setup};
pub use harmos_operations::operations::Scope;
pub use harmos_operations::{
    Backoff, Jitter, OperationDefinition, OperationPolicy, RequestShape, RestartPolicy, RetryPolicy, ServicePolicy,
};
pub use harmos_operations::{Failure, StreamSpec};
pub use operations::refusal::Refusal;
pub use operations::{Catalog, CatalogDescription, Invocation, InvocationExt, Lane, Running};
pub use runtime::{AssemblyError, Handle, Runtime, StartupError};
pub use services::{ServiceContext, ServiceManager};

// Contract Namespaces The Host Does Not Extend, Re-Exported Whole.

pub use harmos_operations::schema;
pub use harmos_operations::{StreamEntry, failure, listeners};
pub use harmos_sdk_derive::{Described, Request, Row};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
extern crate self as harmos_runtime;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T, E = std::convert::Infallible> = std::result::Result<T, Error<E>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[doc(hidden)]
pub mod __private {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_operations::plugin;

    pub use harmos_domain::__private::{
        MapEntry, MessageError, PostcardReader, PostcardValue, decoding_add, decoding_multiply, decoding_vector, postcard_product,
    };
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_operations::__private::CarrierContext;
    pub use harmos_operations::execution::__private::Catalog;
    pub use harmos_operations::inventory as PluginInventory;
    pub use harmos_operations::plugin::{Grant as PluginGrant, Requirement as PluginRequirement, Requirements as PluginRequirements};
    pub use harmos_operations::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};

    // A stream declaration is authored wherever its row type is, host core or
    // no host core, so these three are as unconditional as the attribute that
    // generates them.

    // How generated code names a type the tokens it was handed cannot spell.

    // Ungated on purpose: a hand-written contribution in a crate that compiles

    // no authoring attributes at all still resolves the same paths.

    pub use harmos_operations::__private::Answer;

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    pub extern crate serde;
    pub extern crate serde_json;

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

pub mod catalog {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_operations::catalog::*;

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
