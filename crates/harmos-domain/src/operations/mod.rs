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
#[doc(hidden)]
pub mod payload;

/* === Re-Exports === */

pub use crate::model::operations::policy::{Backoff, FailureKind, Jitter, OperationPolicy, PolicyError, ReplaySafety, RetryPolicy};
pub use crate::model::services::policy::{RestartPolicy, ServicePolicy};
pub use crate::observers;
pub use catalog::{Callees, Declarations, Entry};
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
    Public,
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

    // The operations this one invokes; catalogs aggregate them into plugin requirements.
    const OPERATIONS: &'static [WireDeclaration] = &[];

    fn definition() -> OperationDefinition {
        Entry::of::<Self>().declared().expect("representable operation declaration")
    }

    fn grant(_request: &Self::Request) -> Option<crate::plugin::Grant> {
        None
    }

    fn key(_request: &Self::Request) -> Option<CacheKey> {
        None
    }
}

// The Execution Resource Types An Operation Declares With `resources = [..]`.
pub trait Requires<T: ?Sized> {}

// The Operations An Operation Declares With `operations = [..]`.
pub trait Invokes<T: OperationDeclaration> {}

/* === Implementations === */

impl<'de> Deserialize<'de> for OperationDefinition {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let definition = DecodedOperationDefinition::deserialize(decoder)?;
        definition.validate().map_err(serde::de::Error::custom)?;
        Ok(definition)
    }
}

impl OperationDefinition {
    pub fn named<O: OperationDeclaration + ?Sized>(id: impl Into<String>) -> crate::Result<Self> {
        let id = crate::Identifier::new(id.into())?;
        let mut definition = O::definition();
        if definition.id != id {
            definition.fingerprint = catalog::fingerprint_at::<O>(id.as_str())?;
            definition.id = id;
        }
        definition.validate()?;
        Ok(definition)
    }

    pub fn validate(&self) -> crate::Result<()> {
        if crate::reserved(self.id.as_str()) {
            return Err(crate::Error::ReservedOperation { id: self.id.as_str().to_owned() });
        }
        if !crate::identity::dotted(self.id.as_str()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Published;

    impl OperationDeclaration for Published {
        type Request = ();
        type Response = ();
        type Input = NoStream;
        type Output = NoStream;
        const ID: &'static str = "visibility.public";
        const POLICY: OperationPolicy = crate::model::operations::lifecycle::POLICY;
        const VISIBILITY: Visibility = Visibility::Public;
    }

    #[test]
    fn public_metadata_roundtrip() {
        let contract = WireDeclaration::of::<Published>().contract().unwrap();
        let encoded = serde_json::to_value(&contract).unwrap();
        assert_eq!(encoded["operation"]["visibility"], "public");
        assert_eq!(serde_json::from_value::<Contract>(encoded.clone()).unwrap(), contract);
        assert_eq!(contract.fingerprint(), Published::definition().fingerprint);

        let mut obsolete = encoded;
        obsolete["operation"]["visibility"] = serde_json::json!("shared");
        assert!(serde_json::from_value::<Contract>(obsolete).is_err());

        use crate::catalog::Node;
        use crate::inventory::contracts::{encode_fragments, encoded_fragments_len};
        const OPERATIONS: &[Node<WireDeclaration>] = &[Node::Leaf(WireDeclaration::of::<Published>())];
        const LIFECYCLE: &[OperationPolicy; 2] = &[Published::POLICY; 2];
        const SIZE: usize = encoded_fragments_len("sidecar", "fixture", OPERATIONS, &[], LIFECYCLE);
        const METADATA: [u8; SIZE] = encode_fragments("sidecar", "fixture", OPERATIONS, &[], LIFECYCLE);
        let metadata = crate::artifacts::fragments::<serde_json::Value>(&METADATA, &["contracts"]).unwrap();
        let provided = metadata.iter().find(|fragment| fragment["entry"]["direction"] == "provides").unwrap();
        assert_eq!(provided["entry"]["contract"]["operation"]["visibility"], "public");
    }

    #[test]
    fn grpc_visibility_contracts() {
        let binding = Grpc::new("visibility.v1", "Operations", "Call");
        for visibility in [Visibility::Private, Visibility::Public, Visibility::Host] {
            let mut declaration = WireDeclaration::of::<Published>();
            declaration.operation.visibility = visibility;
            declaration.grpc = Some(binding.clone());
            let result = declaration.contract();
            if visibility == Visibility::Private {
                assert!(matches!(result, Err(ContractError::PrivateGrpc { .. })));
                continue;
            }
            let contract = result.unwrap();
            let encoded = serde_json::to_value(&contract).unwrap();
            assert_eq!(serde_json::from_value::<Contract>(encoded.clone()).unwrap(), contract);
            let mut private = encoded;
            private["operation"]["visibility"] = serde_json::json!("private");
            assert!(serde_json::from_value::<Contract>(private).is_err());
        }
    }
}
