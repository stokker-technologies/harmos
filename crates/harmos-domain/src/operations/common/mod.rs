/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod catalog;
mod context;
mod contract;
mod direction;
mod grpc;
pub mod instance;
mod key;
pub(crate) mod payload;

/* === Re-Exports === */

pub use crate::model::operations::policy::{Backoff, FailureKind, Jitter, OperationPolicy, PolicyError, ReplaySafety, RetryPolicy};
pub use crate::model::services::policy::{RestartPolicy, ServicePolicy};
pub use crate::observers;
pub use catalog::{Declarations, Entry};
pub use context::{CallContext, CallValue, ContextError, ContextValue, PORTABLE_BYTES, PORTABLE_VALUES};
pub use contract::{Codec, Contract, ContractError, Contracts, WireDeclaration};
pub use direction::{Direction, NoStream, Stream, item_types};
pub use grpc::Grpc;
pub use key::{CacheKey, CacheKeyError, MAX_CACHE_KEY_BYTES};

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

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OperationDefinition {
    pub id: Identifier,
    pub description: String,
    pub policy: OperationPolicy,
    pub visibility: Visibility,
    pub replay: ReplaySafety,
    pub fingerprint: crate::Fingerprint,
}

// The decoder constructs the canonical type directly, then checks admission.
// No second public metadata contract or JSON-value staging exists.
#[derive(Deserialize)]
#[serde(remote = "OperationDefinition", deny_unknown_fields)]
struct DecodedOperationDefinition {
    id: Identifier,
    description: String,
    policy: OperationPolicy,
    visibility: Visibility,
    replay: ReplaySafety,
    fingerprint: crate::Fingerprint,
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

pub trait OperationDeclaration: 'static {
    type Request: crate::Portable;
    type Response: crate::Portable;
    type Input: Direction;
    type Output: Direction;

    const ID: &'static str;
    const POLICY: OperationPolicy;
    const DESCRIPTION: &'static str = "";
    const VISIBILITY: Visibility = Visibility::Private;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<Grpc> = None;

    fn definition() -> OperationDefinition {
        OperationDefinition::of::<Self>().expect("valid operation declaration")
    }

    fn grant(_request: &Self::Request) -> Option<crate::plugin::Grant> {
        None
    }

    fn key(_request: &Self::Request) -> Option<CacheKey> {
        None
    }
}

pub trait RequestFor: crate::Portable {
    type Operation: OperationDeclaration<Request = Self>;
}

pub trait Declares {
    type Declaration: OperationDeclaration;
}

/* === Implementations === */

impl<O: OperationDeclaration> Declares for O {
    type Declaration = O;
}

impl<'de> Deserialize<'de> for OperationDefinition {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let definition = DecodedOperationDefinition::deserialize(decoder)?;
        definition.validate().map_err(serde::de::Error::custom)?;
        Ok(definition)
    }
}

impl OperationDefinition {
    pub fn validate(&self) -> crate::Result<()> {
        if crate::reserved(self.id.as_str()) {
            return Err(crate::Error::ReservedOperation { id: self.id.as_str().to_owned() });
        }
        if !crate::identity::segment(self.id.as_str()) {
            return Err(crate::Error::InvalidOperation { id: self.id.to_string() });
        }
        self.policy.validate()?;
        Ok(())
    }

    pub fn of<O: OperationDeclaration + ?Sized>() -> crate::Result<Self> {
        Entry::of::<O>().definition()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
