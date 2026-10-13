/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod payloads;

/* === Re-Exports === */
pub use payloads::*;

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{
    Contract, OperationDeclaration, OperationPolicy, RetryPolicy, Visibility, WireOperation,
};
use crate::plugin::Grant;

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(30),
    retries: RetryPolicy::Never,
};

/* === Macros === */
macro_rules! declarations {
    ($( $(#[$meta:meta])* $name:ident ),* $(,)?) => {
        $(
            $(#[$meta])*
            pub struct $name;

            impl WireOperation for $name {
                const VERSION: u32 = 1;
            }
        )*
    };
}

declarations! {
        Publish,
        RecordSelf,
        RecallSelf,
        Invoke,
        Egress,
        Survey,
}

/* === Type Aliases === */

/* === Structs === */
pub struct Operations;
// A Live Provider Contract. Invocation Still Performs Request-Specific Authorization.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredOperation {
    pub owner: crate::Identifier,
    pub generation: u64,
    pub contract: Contract,
    pub grants: Vec<Grant>,
    pub conditional_access: bool,
    pub grpc_available: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl WireOperation for Operations {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<super::Grpc> =
        Some(super::Grpc::new("harmos.host.v1", "Host", "Operations"));

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: super::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl OperationDeclaration for Operations {
    type Request = OperationsRequest;
    type Response = OperationsResponse;
    const ID: &'static str = "host.operations";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const DESCRIPTION: &'static str =
        "Discover bounded live contracts; invocation authorization remains conditional";
}

impl OperationDeclaration for Publish {
    type Request = Publication;
    type Response = ();
    const ID: &'static str = "host.publish";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl OperationDeclaration for RecordSelf {
    type Request = RecordRequest;
    type Response = Receipt;
    const ID: &'static str = "host.record_self";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl OperationDeclaration for RecallSelf {
    type Request = RecallRequest;
    type Response = RecallResponse;
    const ID: &'static str = "host.recall_self";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl OperationDeclaration for Invoke {
    type Request = InvokeRequest;
    type Response = Receipt;
    const ID: &'static str = "host.invoke";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl OperationDeclaration for Egress {
    type Request = EgressRequest;
    type Response = Fetched;
    const ID: &'static str = "host.egress";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;

    fn grant(request: &EgressRequest) -> Option<Grant> {
        Some(Grant::Egress(Cow::Owned(request.name.clone())))
    }
}

impl OperationDeclaration for Survey {
    type Request = SurveyRequest;
    type Response = SurveyResponse;
    const ID: &'static str = "host.survey";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;

    fn grant(request: &SurveyRequest) -> Option<Grant> {
        Some(Grant::Survey(Cow::Owned(request.stream.clone())))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
