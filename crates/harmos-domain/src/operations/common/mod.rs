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
mod catalog;
mod context;
mod contract;
mod direction;
mod grpc;
pub mod instance;
mod key;
pub(crate) mod payload;
mod policy;
pub(crate) mod response;
mod serving;
#[cfg(feature = "streaming")]
pub mod streaming;
mod surface;

/* === Re-Exports === */
pub use crate::observers;
pub use authored::{EndpointSet, InputEndpoint, Operation, OutputEndpoint};
pub use catalog::{AuthoredDeclarations, Declarations, Entry};
pub use context::{
    CallContext, CallValue, ContextError, ContextValue, PORTABLE_BYTES, PORTABLE_VALUES,
};
pub use contract::{AuthoredContracts, Codec, Contract, ContractError, Contracts, WireDeclaration};
pub use direction::{Direction, NoStream, Stream, item_types};
pub use grpc::Grpc;
pub use instance::Instance;
pub use key::{CacheKey, CacheKeyError, MAX_CACHE_KEY_BYTES};
pub use policy::{
    Backoff, FailureKind, Jitter, OperationPolicy, PolicyError, ReplaySafety, RestartPolicy,
    RetryPolicy,
};
pub use response::Response;
pub use serving::{
    CHILD_METHOD, Serves, ServingError, WIRE_METHOD, WireAnswer, WireBinding, WireChild,
    WireChildAction, WireChildCall, WireChildControl, WireCodecError, WireCommand, WireTable,
    decode_prepared_grant, encode_prepared_grant,
};
#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreamLimits, WireStreams};
#[cfg(feature = "streaming")]
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};
pub use surface::Host;

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::Identifier;

/* === Constants === */
// A deadline no caller reaches: resident and watch policies run until stopped.
pub const UNBOUNDED: std::time::Duration = std::time::Duration::from_secs(1 << 32);

/* === Macros === */
// Declare An Operation On Its Request Type. Fields Keep This Order; Optional
// Ones May Be Omitted. Absent Streams Are `NoStream`.
#[macro_export]
macro_rules! declare {
    ($ty:ty {
        id: $id:expr,
        version: $version:expr,
        policy: $policy:expr,
        response: $response:ty
        $(, input: $input:ty)?
        $(, output: $output:ty)?
        $(, description: $description:expr)?
        $(, visibility: $visibility:expr)?
        $(, replay: $replay:expr)?
        $(, grpc: ($package:expr, $service:expr, $method:expr))?
        $(, grant: $grant:expr)?
        $(, key: $key:expr)?
        $(,)?
    }) => {
        impl $crate::operations::OperationDeclaration for $ty {
            type Response = $response;
            type Input = $crate::declare!(@direction $($input)?);
            type Output = $crate::declare!(@direction $($output)?);
            const ID: &'static str = $id;
            const VERSION: u32 = $version;
            const POLICY: $crate::operations::OperationPolicy = $policy;
            $(const DESCRIPTION: &'static str = $description;)?
            $(const VISIBILITY: $crate::operations::Visibility = $visibility;)?
            $(const REPLAY: $crate::operations::ReplaySafety = $replay;)?
            // A Build Whose Messages Carry No Protobuf Codec Declares No Method.
            $crate::__private::with_protobuf! {
                $(const GRPC: ::core::option::Option<$crate::operations::Grpc> =
                    ::core::option::Option::Some($crate::operations::Grpc::new($package, $service, $method));)?
            }
            $(fn grant(&self) -> ::core::option::Option<$crate::plugin::Grant> {
                ($grant)(self)
            })?
            $(fn key(&self) -> ::core::option::Option<$crate::operations::CacheKey> {
                ($key)(self)
            })?
        }
    };
    (@direction) => { $crate::operations::NoStream };
    (@direction $item:ty) => { $crate::operations::Stream<$item> };
}

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
// The Operation Is Its Request Type. `declare!` (Through `#[operation]`) Is The
// Only Writer Of This Impl; Every Bound A Projection Needs Is Named Here, So One
// Generic Walk Serves The Catalog, gRPC, Descriptor Export And Bindings.
pub trait OperationDeclaration: crate::Portable {
    type Response: crate::Portable;
    type Input: Direction;
    type Output: Direction;

    const ID: &'static str;
    const VERSION: u32;
    const POLICY: OperationPolicy;
    const DESCRIPTION: &'static str = "";
    const VISIBILITY: Visibility = Visibility::Private;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<Grpc> = None;

    fn grant(&self) -> Option<crate::plugin::Grant> {
        None
    }

    fn key(&self) -> Option<CacheKey> {
        None
    }
}

pub trait Declares {
    type Declaration: OperationDeclaration;
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
