/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_contract::operations::{
    Backoff, Jitter, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
    WireOperation,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Stage {
    pub id: String,
}
pub struct Read;

pub struct Counts;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Value {
    pub value: String,
}

/* === Enums === */
#[cfg_attr(feature = "grpc", harmos_guest::operation(request = u32, response = u32, input = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Guest", method = "Upload")))]
#[cfg_attr(not(feature = "grpc"), harmos_guest::operation(request = u32, response = u32, input = u32, policy = Read::POLICY, version = 1))]
pub struct Upload;

#[cfg_attr(feature = "grpc", harmos_guest::operation(request = u32, response = u32, output = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Guest", method = "Download")))]
#[cfg_attr(not(feature = "grpc"), harmos_guest::operation(request = u32, response = u32, output = u32, policy = Read::POLICY, version = 1))]
pub struct Download;

#[cfg_attr(feature = "grpc", harmos_guest::operation(request = u32, response = u32, input = u32, output = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Guest", method = "Exchange")))]
#[cfg_attr(not(feature = "grpc"), harmos_guest::operation(request = u32, response = u32, input = u32, output = u32, policy = Read::POLICY, version = 1))]
pub struct Exchange;

#[harmos_guest::catalog(harmos_path = ::harmos_guest)]
pub enum Streaming {
    Upload(Upload),
    Download(Download),
    Exchange(Exchange),
}

/* === Traits === */

/* === Implementations === */
impl harmos_guest::operations::ContextValue for Stage {
    const KEY: &'static str = "fixture.stage";
}
impl WireOperation for Read {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    fn visit_grpc<V: harmos_contract::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }

    #[cfg(feature = "grpc")]
    const GRPC: Option<harmos_contract::operations::Grpc> =
        Some(harmos_contract::operations::Grpc {
            package: std::borrow::Cow::Borrowed("fixture.v1"),
            service: std::borrow::Cow::Borrowed("Guest"),
            method: std::borrow::Cow::Borrowed("Read"),
        });
}

impl OperationDeclaration for Read {
    type Request = Value;
    type Response = Value;
    const ID: &'static str = "fixture.read";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
        restart: RestartPolicy::Never,
    };
}

impl WireOperation for Counts {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Counts {
    type Request = Value;
    type Response = Value;
    const ID: &'static str = "fixture.counts";
    const POLICY: OperationPolicy = Read::POLICY;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
