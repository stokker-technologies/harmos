/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod authored;
mod capability;
mod catalog;
mod context;
mod contract;
mod grpc;
pub mod host;
pub mod instance;
mod key;
pub mod lifecycle;
mod policy;
pub(crate) mod response;
mod serving;
#[cfg(feature = "streaming")]
pub mod streaming;

/* === Re-Exports === */
pub use crate::observers;
pub use authored::{EndpointSet, InputEndpoint, Operation, OutputEndpoint};
pub use capability::{Callable, Host};
pub use catalog::{AuthoredDeclarations, Declarations, Entry};
pub use context::{
    CallContext, CallValue, ContextError, ContextValue, PORTABLE_BYTES, PORTABLE_VALUES,
};
pub use contract::{
    AuthoredContracts, Codec, Contract, ContractError, Contracts, WireDeclaration, WireOperation,
    WireVisitor,
};
pub use grpc::Grpc;
#[cfg(feature = "grpc")]
pub use grpc::GrpcVisitor;
pub use instance::Instance;
pub use key::{CacheKey, CacheKeyError, MAX_CACHE_KEY_BYTES};
pub use policy::{
    Backoff, Failure, Jitter, OperationPolicy, PolicyError, ReplaySafety, RetryPolicy,
};
pub use response::Response;
pub use serving::{
    CHILD_METHOD, Serves, ServingError, WIRE_METHOD, WireAnswer, WireBinding, WireChild,
    WireChildAction, WireChildCall, WireChildControl, WireCodecError, WireCommand, WireTable,
};
#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreamLimits, WireStreams};
#[cfg(feature = "streaming")]
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::Identifier;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Definition {
    pub id: Identifier,
    pub description: String,
    pub policy: OperationPolicy,
    pub visibility: Visibility,
    pub replay: ReplaySafety,
}

// The decoder constructs the canonical type directly, then checks admission.
// No second public metadata contract or JSON-value staging exists.
#[derive(Deserialize)]
#[serde(remote = "Definition", deny_unknown_fields)]
struct DecodedDefinition {
    id: Identifier,
    description: String,
    policy: OperationPolicy,
    visibility: Visibility,
    replay: ReplaySafety,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Shared,
    Host,
}

/* === Traits === */
pub trait OperationDeclaration: Send + Sync + 'static {
    type Request: Send + 'static;
    type Response: Send + Sync + 'static;

    const ID: &'static str;
    const POLICY: OperationPolicy;
    const DESCRIPTION: &'static str = "";
    const VISIBILITY: Visibility = Visibility::Private;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;

    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, None)
    }

    fn visit_wire<V: WireVisitor>(_visitor: V) -> Option<V::Entry> {
        None
    }

    fn grant(_request: &Self::Request) -> Option<crate::plugin::Grant> {
        None
    }

    fn key(_request: &Self::Request) -> Option<CacheKey> {
        None
    }
}

pub trait Declares {
    type Declaration: OperationDeclaration;
}

pub trait InputDeclaration: OperationDeclaration {
    type Item: Send + 'static;
}

pub trait OutputDeclaration: OperationDeclaration {
    type Item: Send + 'static;
}

/* === Implementations === */
impl<O: OperationDeclaration> Declares for O {
    type Declaration = O;
}

impl<'de> Deserialize<'de> for Definition {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let definition = DecodedDefinition::deserialize(decoder)?;
        definition.validate().map_err(serde::de::Error::custom)?;
        Ok(definition)
    }
}

impl Definition {
    pub fn validate(&self) -> crate::Result<()> {
        if crate::reserved(self.id.as_str()) {
            return Err(crate::Error::ReservedOperation {
                id: self.id.as_str().to_owned(),
            });
        }
        self.policy.validate()?;
        Ok(())
    }

    pub fn of<O: OperationDeclaration>() -> crate::Result<Self> {
        Entry::of::<O>().definition()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
