/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Share The External gRPC Benchmark Contract With Its Fixture Server.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos_runtime::operations::{Handler, RetryPolicy, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;

#[harmos_runtime::operation(id = "benchmark_echo",  response = Payload,
    visibility = Visibility::Shared,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30), attempt_timeout: Duration::from_secs(30),
        retries: RetryPolicy::Never,
    },
    grpc(package = "benchmark.v1", service = "Commands", method = "Echo"),
)]
pub struct EchoDeclaration {
    pub bytes: Vec<u8>,
}

#[harmos_runtime::message]
pub struct Payload {
    pub bytes: Vec<u8>,
}

pub struct Echoing;

/* === Enums === */

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Application for Model {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl Handler for Echoing {
    type Request = EchoDeclaration;
    type Response = <EchoDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Infallible;
    type StreamEndpoints = ();

    async fn initialize(_context: harmos_runtime::OperationContext) -> Result<Self, Infallible> {
        Ok(Self)
    }

    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut EchoDeclaration, _: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Payload>, Infallible> {
        let bytes = std::mem::take(&mut request.bytes);
        Ok(Payload { bytes }.into())
    }
}

/* === Public Functions === */

pub fn wire_settings() -> harmos_runtime::grpc::server::Settings {
    let bytes = nz(16 * 1024 * 1024);
    let message = nz(4 * 1024 * 1024);
    let direction = harmos_runtime::operations::StreamSettings { items: nz(16), bytes };
    harmos_runtime::grpc::server::Settings {
        calls: nz(64),
        decode: harmos_runtime::grpc::DecodeSettings {
            message_bytes: message,
            retained_bytes: bytes,
        },
        encode: harmos_runtime::grpc::EncodeSettings {
            message_bytes: message,
            buffer_bytes: nz(8192),
        },
        input: direction,
        output: direction,
        opening_timeout: Duration::from_secs(5),
    }
}

pub fn settings() -> harmos_runtime::grpc::Settings {
    harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        operations: wire_settings(),
        reflection: false,
        web: false,
        allowed_origins: Vec::new(),
        tls: None,
        connections: nz(4),
        calls_per_connection: nz(8),
        request_timeout: Duration::from_secs(30),
        shutdown_timeout: Duration::from_secs(30),
    }
}

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
