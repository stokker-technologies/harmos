/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) use harmos_storage::aggregate;
pub mod builtin;
pub mod config;
mod error;
pub mod filesystem;
#[cfg(feature = "grpc")]
pub mod grpc;
pub mod prelude;
pub mod storage;

mod application;

pub mod evidence;

pub mod state;

pub mod records;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin"))]
pub mod plugin;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin-sidecar"))]
pub mod sidecar;

#[cfg(all(not(target_arch = "wasm32"), feature = "plugin-guest"))]
pub mod guest;

pub mod operations;

pub mod runtime;

#[cfg(feature = "simulation")]
pub mod simulation;

pub mod streams;

pub mod services;

pub mod observers;

mod supervision;

pub mod __private {
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_domain::plugin::schema_markers as PluginSchemaMarkers;
    pub extern crate serde;
    pub extern crate serde_json;
    pub use harmos_domain::__private::CarrierContext;
    pub use harmos_domain::__private::{
        MessageError, PostcardReader, PostcardValue, decoding_add, decoding_multiply, decoding_vector, operation_identity, owner_segment,
        postcard_product, stream_identity,
    };

    pub use harmos_domain::inventory as PluginInventory;
    pub use harmos_domain::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};
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

    pub use crate::state::{Catalogued, Mutual};
    pub use harmos_domain::__private::Answer;

    pub fn stream_definition<S: StreamRow>(id: &'static str, version: u32, window: usize) -> crate::streams::Definition {
        crate::streams::Definition::declared::<S>(id, version, window)
            .batching(S::batch_definition())
            .presentation(S::metadata(), S::row_metadata())
    }
}

/* === Re-Exports === */

pub use harmos_domain::model;
pub use harmos_operations as builtins;

pub use application::Application;
pub use error::{BackendFailure, Error};
pub use harmos_domain::{DefinitionId, Identified};

#[cfg(feature = "signal")]
pub use harmos_storage::signal;

pub use harmos_domain::{
    Backoff, Digest, Fingerprint, Jitter, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, QualifiedId, RestartPolicy,
    RetryPolicy, ServicePolicy,
};
pub use harmos_sdk::operations::{EndpointSet, Response};
pub use harmos_sdk::{
    Context, Dependencies, Dependent, Executable, ExecutionContext, Operation, RegistrationBatch, Resource, ResourceError, Service,
};
pub use operations::refusal::Refusal;
pub use operations::{Invocation, Lane, Registration, RegistrationExt, Running};
pub use runtime::{Builder, Handle, Runtime};
pub use services::{ServiceContext, ServiceManager};

pub use harmos_domain::{
    Aggregate, Failure, Identifier, IdentifierError, MemorySize, Message, MessageError, Metadata, RowMetadata, Storable, StreamField,
    StreamSpec, Value, ValueError, btree_map_memory,
};
pub use harmos_sdk::plugin::Plugin;
// Contract Namespaces The Host Does Not Extend, Re-Exported Whole.

// `message` Names Both The Contract Module And Its Same-Named Attribute.
pub use harmos_domain::{artifacts, catalog, failure, listeners, message, schema};
pub use harmos_macros::{lanes, listener, observer, operation, plugin, record, records, stream, streams, transaction, transactions};

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

/* === Tests === */
