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
#[cfg(feature = "grpc")]
use std::time::Duration;
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};
// Third Party Imports
// Local Imports
use super::{Counts, ReadDeclaration};
#[cfg(feature = "grpc")]
use super::{Lanes, Managed, source};
#[cfg(feature = "grpc")]
use harmos_runtime::grpc::{Codec, InputEnvelope, OutputEnvelope};
use harmos_runtime::operations::{Operation, StreamInput, StreamOutput, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

macro_rules! serving {
    ($owner:ident, $declaration:ident, $endpoints:ty, $request:ident, $ports:ident, $run:block) => {
        struct $owner(Arc<Counts>);
        impl Operation for $owner {
            type Request = <$declaration as harmos_runtime::OperationDeclaration>::Request;
            type Response = <$declaration as harmos_runtime::OperationDeclaration>::Response;

            type Error = harmos_runtime::Error;
            type Endpoints = $endpoints;
            async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
                let counts = context.dependency::<Counts>().expect("fixture dependency");
                counts.prepared.fetch_add(1, Ordering::SeqCst);
                counts.operation_initialized.fetch_add(1, Ordering::SeqCst);
                Ok(Self(counts))
            }
            async fn run(
                &mut self, _: harmos_runtime::OperationContext, $request: &mut Self::Request, $ports: &mut Self::Endpoints,
            ) -> Result<harmos_runtime::operations::Response<u32>, Self::Error> {
                let result: ::core::result::Result<u32, Self::Error> = async move {
                    self.0.runs.fetch_add(1, Ordering::SeqCst);
                    $run
                }
                .await;
                result.map(::core::convert::Into::into)
            }
            async fn stop(&mut self) -> Result<(), Self::Error> {
                self.0.stopped.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            async fn cleanup(&mut self) -> Result<(), Self::Error> {
                self.0.finalized.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[harmos_runtime::operation(request = Upload, infer_request, id = "upload",

    policy = ReadDeclaration::POLICY,
    response = u32,
    input = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Upload"),
)]
pub(super) struct UploadDeclaration;

#[harmos_runtime::message]

pub(super) struct Upload {
    pub(super) value: u32,
}

#[harmos_runtime::operation(request = Download, infer_request, id = "download",

    policy = ReadDeclaration::POLICY,
    response = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Download"),
)]
pub(super) struct DownloadDeclaration;

#[harmos_runtime::message]

pub(super) struct Download {
    pub(super) value: u32,
}

// Field tags derive from field names, so this carries a string on `Download`'s `value` tag.
#[cfg(feature = "grpc")]
#[harmos_runtime::message]
struct WrongShape {
    value: String,
}

#[harmos_runtime::operation(request = Exchange, infer_request, id = "exchange",

    policy = ReadDeclaration::POLICY,
    response = u32,
    input = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Exchange"),
)]
pub(super) struct ExchangeDeclaration;

#[harmos_runtime::message]

pub(super) struct Exchange {
    pub(super) value: u32,
}

#[harmos_runtime::catalog]
pub(super) enum Operations {
    Upload(UploadDeclaration),
    Download(DownloadDeclaration),
    Exchange(ExchangeDeclaration),
}

// Runtime-error endpoints bind through the same catalog projection as
// `bind_operations`, which is the authored sidecar route.

/* === Traits === */

/* === Implementations === */

serving!(Uploading, UploadDeclaration, StreamInput<u32>, request, input, {
    let mut total = request.value;
    while let Some(item) = input.recv().await {
        total += *item;
    }
    Ok(total)
});
serving!(Downloading, DownloadDeclaration, StreamOutput<u32>, request, output, {
    for value in [0, request.value] {
        output.send_with(4, || Ok(value)).await?;
    }
    Ok(request.value)
});
serving!(Exchanging, ExchangeDeclaration, (StreamInput<u32>, StreamOutput<u32>), request, ports, {
    let mut total = request.value;
    while let Some(item) = ports.0.recv().await {
        total += *item;
        ports.1.send_with(4, || Ok(total)).await?;
    }
    Ok(total)
});

/* === Public Functions === */

pub(super) fn bind(table: &mut WireTable, counts: Arc<Counts>) -> Result<(), harmos_runtime::operations::ServingError> {
    let direction = StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(64).unwrap() };
    let limits = WireStreamLimits { input: direction, output: direction };
    table.stream_limits(limits)?;
    let mut dependencies = harmos_runtime::Dependencies::new();
    dependencies.insert(counts).unwrap();
    table.register(
        harmos_runtime::RegistrationBatch::new(dependencies)
            .implementation::<UploadDeclaration, Uploading>()
            .implementation::<DownloadDeclaration, Downloading>()
            .implementation::<ExchangeDeclaration, Exchanging>(),
    )
}

/* === Private Functions === */

#[cfg(feature = "grpc")]
fn opening<R: Send + Sync + 'static>(
    request: R,
) -> impl futures_core::Stream<Item = harmos_runtime::operations::Response<InputEnvelope<R, u32>>> + Send + 'static {
    futures_util::stream::iter([
        harmos_runtime::operations::Response::new(InputEnvelope::Request(request)),
        harmos_runtime::operations::Response::new(InputEnvelope::Item(0)),
        harmos_runtime::operations::Response::new(InputEnvelope::Item(7)),
    ])
}

/* === Tests === */

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_stream_lifecycle() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.initialize().await.unwrap();
    assert_eq!(fixture.runtime.grpc().methods().len(), 4);
    let operations = harmos_runtime::grpc::server::Settings::default();
    let attached = fixture
        .runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            operations,
            reflection: true,
            web: false,
            allowed_origins: Vec::new(),
            tls: None,
            connections: NonZeroUsize::new(4).unwrap(),
            calls_per_connection: NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let upload: tonic::Response<harmos_runtime::operations::StreamItem<u32>> = client
        .client_streaming(
            tonic::Request::new(opening(Upload { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Upload"),
            Codec::<harmos_runtime::grpc::Transcoded, u32, harmos_runtime::operations::Response<InputEnvelope<Upload, u32>>>::new(
                operations.decode,
                operations.encode,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(*upload.into_inner(), 12);

    client.ready().await.unwrap();
    let mut download = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new(Download { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<Download>>::new(
                operations.decode,
                operations.encode,
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    for expected in [OutputEnvelope::Item(0), OutputEnvelope::Item(5), OutputEnvelope::Response(5)] {
        assert_eq!(*download.message().await.unwrap().unwrap(), expected);
    }
    assert!(download.message().await.unwrap().is_none());

    client.ready().await.unwrap();
    let mut exchange = client
        .streaming(
            tonic::Request::new(opening(Exchange { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Exchange"),
            Codec::<
                harmos_runtime::grpc::Transcoded,
                OutputEnvelope<u32, u32>,
                harmos_runtime::operations::Response<InputEnvelope<Exchange, u32>>,
            >::new(operations.decode, operations.encode)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    for expected in [OutputEnvelope::Item(5), OutputEnvelope::Item(12), OutputEnvelope::Response(12)] {
        assert_eq!(*exchange.message().await.unwrap().unwrap(), expected);
    }
    assert!(exchange.message().await.unwrap().is_none());
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 3);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 3);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 3);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 3);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 3);
    fixture.shutdown().await.unwrap();
    assert!(fixture.runtime.grpc().methods().is_empty());
    fixture.runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_shape_refusal() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.initialize().await.unwrap();
    let operations = harmos_runtime::grpc::server::Settings::default();
    let attached = fixture
        .runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            operations,
            reflection: false,
            web: false,
            allowed_origins: Vec::new(),
            tls: None,
            connections: NonZeroUsize::new(4).unwrap(),
            calls_per_connection: NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();

    // The Declared `value` Field Carries A String Where The Declaration Expects A u32.
    let refused = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new(WrongShape { value: "not a number".to_owned() })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<WrongShape>>::new(
                operations.decode,
                operations.encode,
            )
            .unwrap(),
        )
        .await;
    let status = match refused {
        Ok(response) => match response.into_inner().message().await {
            Err(status) => status,
            Ok(_) => panic!("a wrong-shape request must be refused"),
        },
        Err(status) => status,
    };
    assert_eq!(status.code(), tonic::Code::InvalidArgument, "{status:?}");
    // The Status Details Carry The Structured Failure Unchanged
    let failure = harmos_runtime::Failure::decode(status.details()).unwrap();
    assert_eq!(status.message(), failure.cause(), "{status:?}");
    assert!(failure.cause().contains("wire type mismatch"), "{status:?}");
    assert!(failure.context().operation().is_some(), "{status:?}");
    assert!(failure.diagnostics().is_none(), "{status:?}");
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}
