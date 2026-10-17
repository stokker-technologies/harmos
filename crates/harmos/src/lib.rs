/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod aggregate;
pub mod builtin;
pub mod config;
mod error;
#[cfg(feature = "grpc")]
pub mod grpc;
mod identity;
#[cfg(all(feature = "packaging", not(target_arch = "wasm32")))]
pub mod packaging;
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

#[cfg(feature = "signal")]
pub mod signal;

pub mod streams;

pub mod services;

pub mod observers;

mod supervision;

pub mod __private {
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_domain::plugin::schema_markers as PluginSchemaMarkers;
    pub extern crate serde;
    pub extern crate serde_json;
    pub use crate::with_protobuf;
    pub use harmos_domain::__private::CarrierContext;
    pub use harmos_domain::__private::{
        DecodeInput, DecodeScalar, DecodeValue, MessageError, PostcardReader, PostcardValue,
        decoding_add, decoding_multiply, decoding_vector,
    };
    #[cfg(feature = "grpc")]
    pub use harmos_domain::__private::{Protobuf, prost};
    pub use harmos_domain::inventory as PluginInventory;
    pub use harmos_domain::plugin::{
        Contribution as PluginContribution, Declared as PluginDeclared, Grant as PluginGrant,
        Requirement as PluginRequirement, Requirements as PluginRequirements,
    };
    pub use harmos_domain::{
        Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError,
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

    pub fn stream_definition<S: StreamRow>(
        id: &'static str,
        version: u32,
        window: usize,
    ) -> crate::streams::Definition {
        crate::streams::Definition::declared::<S>(id, version, window)
            .batching(S::batch_definition())
            .presentation(S::metadata(), S::row_metadata())
    }
}

/* === Re-Exports === */
pub use application::Application;
pub use error::{BackendFailure, Error};
#[cfg(feature = "bindings-python")]
pub use harmos_domain::bindings;
pub use identity::{DefinitionId, Identified};
pub use operations::refusal::Refusal;
pub use runtime::{Builder, Runtime};

#[cfg(feature = "grpc")]
pub use harmos_domain::Protobuf;
pub use harmos_domain::plugin::Plugin;
pub use harmos_domain::{
    Aggregate, Failure, Identifier, IdentifierError, MemorySize, Message, MessageError, Metadata,
    RowMetadata, Storable, StreamField, StreamSpec, Value, ValueError, btree_map_memory,
};
// Contract Namespaces The Host Does Not Extend, Re-Exported Whole.
#[cfg(any(feature = "plugin", feature = "packaging"))]
pub use harmos_bundle as bundle;
#[cfg(all(feature = "reflection", not(feature = "grpc")))]
pub use harmos_domain::grpc;
// `message` Names Both The Contract Module And Its Same-Named Attribute.
pub use harmos_domain::{artifacts, catalog, failure, listeners, message, schema};
pub use harmos_macros::{
    lanes, listener, observer, operation, plugin, record, records, stream, streams, transaction,
    transactions,
};

/* === Imports === */
// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
// Local Imports

/* === Constants === */

/* === Macros === */
// Generated And Native Message Implementations Follow The Same Facade Feature.
// Another Consumer May Independently Enable The Portable Contract's Codec.
#[cfg(feature = "grpc")]
#[macro_export]
macro_rules! with_protobuf { ($($tokens:tt)*) => { $($tokens)* }; }

#[cfg(not(feature = "grpc"))]
#[macro_export]
macro_rules! with_protobuf {
    ($($tokens:tt)*) => {};
}

/* === Type Aliases === */
pub type Result<T, E = std::convert::Infallible> = std::result::Result<T, Error<E>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
