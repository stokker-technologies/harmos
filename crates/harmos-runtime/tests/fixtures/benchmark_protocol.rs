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
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "benchmark.echo", version = 1, response = Payload,
    visibility = Visibility::Shared,
    policy = OperationPolicy {
        timeout: Duration::from_secs(30), attempt_timeout: Duration::from_secs(30),
        retries: RetryPolicy::Never, restart: RestartPolicy::Never,
    },
    grpc(package = "benchmark.v1", service = "Commands", method = "Echo"),
)]
pub struct Echo {
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
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl Operation for Echoing {
    type Declaration = Echo;
    type Resources = ();
    type Error = Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Infallible> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut Echo, _: &mut Self::Endpoints,
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
        decode: harmos_runtime::grpc::DecodeSettings { message_bytes: message, retained_bytes: bytes },
        encode: harmos_runtime::grpc::EncodeSettings { message_bytes: message, buffer_bytes: nz(8192) },
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
