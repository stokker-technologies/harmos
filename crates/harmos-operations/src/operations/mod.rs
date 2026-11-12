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
mod error;
mod grpc;
pub mod instance;
#[doc(hidden)]
pub mod payload;
mod protobuf;
mod specification;

/* === Re-Exports === */

pub use crate::observers;
pub use catalog::{Callees, Declarations, Entry};
pub use context::{CallContext, CallValue, ContextError, ContextValue, PORTABLE_BYTES, PORTABLE_VALUES};
pub use contract::{Codec, Contract, ContractError, Contracts, WireDeclaration};
pub use direction::{Direction, NoStream, Stream, item_types};
pub use error::{Error, Result};
pub use grpc::Grpc;
pub use harmos_protocol::v1::Protobuf;
pub use harmos_protocol::policy::{Backoff, FailureKind, Jitter, OperationPolicy, PolicyError, ReplaySafety, RetryPolicy};
pub use harmos_protocol::service::{RestartPolicy, ServicePolicy};
pub use protobuf::ProtobufExt;
pub use specification::{OperationSpec, RequestShape};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos_domain::Identifier;

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
    pub fingerprint: harmos_domain::Fingerprint,
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
    fingerprint: harmos_domain::Fingerprint,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Callable by the declaring plugin and the host.
    Private,

    /// Callable by everyone connected, subject to client authorization.
    Public,
}

/* === Traits === */

// The Execution Resource Types An Operation Declares With `resources = [..]`.

// The Operations An Operation Declares With `operations = [..]`.

/* === Implementations === */

impl<'de> Deserialize<'de> for OperationDefinition {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let definition = DecodedOperationDefinition::deserialize(decoder)?;
        definition.validate().map_err(serde::de::Error::custom)?;
        Ok(definition)
    }
}

impl OperationDefinition {
    pub fn validate(&self) -> crate::operations::Result<()> {
        if crate::reserved(self.id.as_str()) {
            return Err(crate::operations::Error::ReservedOperation { id: self.id.as_str().to_owned() });
        }
        if !harmos_domain::identity::dotted(self.id.as_str()) {
            return Err(crate::operations::Error::InvalidOperation { id: self.id.to_string() });
        }
        self.policy.validate()?;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    struct Published;

    impl crate::operations::RequestShape for Published {
        type Request = ();
        type Response = ();
        type Input = NoStream;
        type Output = NoStream;
    }
    static PUBLISHED_DESCRIPTION: crate::operations::WireDeclaration =
        crate::operations::WireDeclaration::request::<Published>("visibility.public", crate::model::operations::lifecycle::POLICY)
            .visibility(Visibility::Public);

    const PUBLISHED_OPERATION: crate::operations::OperationSpec<Published> =
        crate::operations::OperationSpec::<Published>::new(&PUBLISHED_DESCRIPTION);

    #[test]
    fn public_metadata_roundtrip() {
        let contract = PUBLISHED_OPERATION.wire().contract().unwrap();
        let encoded = serde_json::to_value(&contract).unwrap();
        assert_eq!(encoded["operation"]["visibility"], "public");
        assert_eq!(serde_json::from_value::<Contract>(encoded.clone()).unwrap(), contract);
        assert_eq!(contract.fingerprint(), PUBLISHED_OPERATION.definition().expect("valid operation").fingerprint);

        for visibility in ["shared", "host"] {
            let mut obsolete = encoded.clone();
            obsolete["operation"]["visibility"] = serde_json::json!(visibility);
            assert!(serde_json::from_value::<Contract>(obsolete).is_err());
        }
    }

    #[test]
    fn grpc_visibility_contracts() {
        let binding = Grpc::new("visibility.v1", "Operations", "Call");
        for visibility in [Visibility::Private, Visibility::Public] {
            let mut declaration = PUBLISHED_OPERATION.wire();
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
