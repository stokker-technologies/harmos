/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

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

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin"))]
pub mod plugin;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin-sidecar"))]
pub mod sidecar;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin-guest"))]
pub mod guest;
pub mod operations;
pub mod runtime;

pub mod observers;
pub mod services;
#[cfg(feature = "simulation")]
pub mod simulation;
pub mod streams;
mod supervision;

/* === Re-Exports === */

pub use application::{Application, Startup};
pub use error::{BackendFailure, Error};
pub use harmos_domain as domain;
pub use harmos_domain::model;
pub use harmos_domain::{DefinitionId, Identified};
pub use harmos_operations as builtins;
pub(crate) use harmos_storage::aggregate;

pub use harmos_domain::{
    Aggregate, Failure, Identifier, IdentifierError, MemorySize, Message, MessageError, Metadata, RowMetadata, Storable, StreamField,
    StreamSpec, Value, ValueError, btree_map_memory,
};
pub use harmos_domain::{
    Backoff, Digest, Fingerprint, Jitter, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, QualifiedId, RestartPolicy,
    RetryPolicy, ServicePolicy,
};
pub use harmos_sdk::operations::{EndpointSet, OperationHost, Response};
pub use harmos_sdk::{Context, Dependent, Executable, ExecutionContext, Lifecycle, Operation, Resource, ResourceError, Service, Setup};
#[cfg(feature = "signal")]
pub use harmos_storage::signal;
pub use operations::refusal::Refusal;
pub use operations::{Catalog, Invocation, InvocationExt, Lane, Running};
pub use runtime::{Handle, Runtime, StartupError};
pub use services::{ServiceContext, ServiceManager};

#[cfg(feature = "plugin-sidecar")]
pub use harmos_macros::plugin as sidecar;

#[cfg(feature = "plugin-guest")]
pub use harmos_macros::plugin as guest;
pub use harmos_sdk::plugin::Plugin;

// Contract Namespaces The Host Does Not Extend, Re-Exported Whole.

// `message` Names Both The Contract Module And Its Same-Named Attribute.
pub use harmos_domain::{artifacts, failure, listeners, message, schema};
pub use harmos_macros::{
    lanes, listener, observer, plugin, record, records, runtime_catalog as catalog, runtime_operation as operation, stream, streams,
    transaction, transactions,
};

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

    pub use harmos_domain::__private::CarrierContext;
    pub use harmos_domain::__private::{
        MapEntry, MessageError, PostcardReader, PostcardValue, decoding_add, decoding_multiply, decoding_vector, operation_identity,
        owner_segment, postcard_product, stream_identity,
    };
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_domain::inventory as PluginInventory;
    pub use harmos_domain::plugin::schema_markers as PluginSchemaMarkers;
    pub use harmos_domain::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};
    pub use harmos_sdk::__private::RegistrationBatch;
    pub use harmos_sdk::plugin::{
        Contribution as PluginContribution, Declared as PluginDeclared, Grant as PluginGrant, Requirement as PluginRequirement,
        Requirements as PluginRequirements,
    };

    #[cfg(feature = "plugin-sidecar")]
    pub use harmos_sidecar as sidecar;

    // A stream declaration is authored wherever its row type is, host core or
    // no host core, so these three are as unconditional as the attribute that
    // generates them.
    pub use harmos_domain::streams::{BatchDefinition, StreamCatalog, StreamRow};

    // How generated code names a type the tokens it was handed cannot spell.

    // Ungated on purpose: a hand-written contribution in a crate that compiles

    // no authoring attributes at all still resolves the same paths.
    pub use crate::state::{Catalogued, Mutual, Paired};
    pub use harmos_domain::__private::Answer;

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

    pub fn stream_definition<S: StreamRow>(id: &'static str, window: usize) -> crate::streams::Definition {
        crate::streams::Definition::declared::<S>(id, 1, window)
            .batching(S::batch_definition())
            .presentation(S::metadata(), S::row_metadata())
    }

    /* === Private Functions === */

    /* === Tests === */
}

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

/* === Tests === */
