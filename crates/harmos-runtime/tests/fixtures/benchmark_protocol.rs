/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Share The External gRPC Benchmark Contract With Its Fixture Server.

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos_runtime::operations::RetryPolicy;

/* === Structs === */

pub struct Model;

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Payload)]
#[derive(Clone)]
pub struct EchoDeclaration {
    pub bytes: Vec<u8>,
}

pub static ECHO_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<EchoDeclaration>(
        "benchmark.echo",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(30),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    )
    .visibility(harmos_runtime::operations::Visibility::Public)
    .grpc(harmos_runtime::operations::Grpc::new("benchmark.v1", "Commands", "Echo"));

pub const ECHO_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<EchoDeclaration> =
    harmos_runtime::operations::OperationSpec::<EchoDeclaration>::new(&ECHO_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
pub struct Payload {
    pub bytes: Vec<u8>,
}

/* === Enums === */

pub fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(echo_declaration_operation().expect("representable operation"));
    batch
}

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

pub async fn echo_declaration_handler(
    mut request: EchoDeclaration, _: harmos_runtime::operations::FunctionContext<EchoDeclaration>,
) -> Result<harmos_runtime::operations::Response<Payload>, Infallible> {
    let bytes = std::mem::take(&mut request.bytes);
    Ok(Payload { bytes }.into())
}

pub fn echo_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(ECHO_DECLARATION_OPERATION, const { &[] }, echo_declaration_handler, |_| {
        harmos_runtime::operations::FailureKind::Permanent
    })
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
