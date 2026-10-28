/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[cfg(feature = "grpc")]
use std::time::Duration;
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};

use super::{Counts, ReadDeclaration};
#[cfg(feature = "grpc")]
use super::{Lanes, Managed, source};
#[cfg(feature = "grpc")]
use harmos_runtime::grpc::{Codec, InputEnvelope, OutputEnvelope};
use harmos_runtime::operations::{StreamSettings, WireStreamLimits};
use harmos_sdk::__private::WireTable;

macro_rules! serving {
    ($declaration:ident, $request:ident, $ports:ident, $run:block) => {
        impl harmos_runtime::Operation for $declaration {
            type Error = harmos_runtime::Error;

            async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
                let counts = ctx.resource::<Counts>();
                counts.started.fetch_add(1, Ordering::SeqCst);
                counts.runs.fetch_add(1, Ordering::SeqCst);
                let _guard = AttemptGuard(counts);
                let $request = self;
                let $ports = ctx.streams();
                let result: Result<u32, Self::Error> = async move { $run }.await;
                result.map(Into::into)
            }
        }
    };
}

struct AttemptGuard(Arc<Counts>);

#[harmos_runtime::operation(resources = [Counts], id = "device.upload",
    policy = ReadDeclaration::POLICY,
    response = u32,
    input = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Upload"),
)]
pub(super) struct UploadDeclaration {
    pub(super) value: u32,
}

#[harmos_runtime::operation(resources = [Counts], id = "device.download",
    policy = ReadDeclaration::POLICY,
    response = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Download"),
)]
pub(super) struct DownloadDeclaration {
    pub(super) value: u32,
}

// Field tags derive from field names, so this carries a string on `DownloadDeclaration`'s `value` tag.
#[cfg(feature = "grpc")]
#[harmos_runtime::message]
struct WrongShape {
    value: String,
}

#[harmos_runtime::operation(resources = [Counts], id = "device.exchange",
    policy = ReadDeclaration::POLICY,
    response = u32,
    input = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Exchange"),
)]
pub(super) struct ExchangeDeclaration {
    pub(super) value: u32,
}

#[harmos_runtime::catalog]
pub(super) enum Operations {
    UploadDeclaration(UploadDeclaration),
    DownloadDeclaration(DownloadDeclaration),
    ExchangeDeclaration(ExchangeDeclaration),
}

// Runtime-Error Endpoints Use The Plugin's Constant Catalog

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
    }
}

serving!(UploadDeclaration, request, input, {
    let mut total = request.value;
    while let Some(item) = input.recv().await {
        total += *item;
    }
    Ok(total)
});
serving!(DownloadDeclaration, request, output, {
    for value in [0, request.value] {
        output.send_with(4, || Ok(value)).await?;
    }
    Ok(request.value)
});
serving!(ExchangeDeclaration, request, ports, {
    let mut total = request.value;
    while let Some(item) = ports.0.recv().await {
        total += *item;
        ports.1.send_with(4, || Ok(total)).await?;
    }
    Ok(total)
});

pub(super) fn configure(table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
    let direction = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(64).unwrap(),
    };
    let limits = WireStreamLimits { input: direction, output: direction };
    table.stream_limits(limits)
}

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
            tonic::Request::new(opening(UploadDeclaration { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Upload"),
            Codec::<harmos_runtime::grpc::Transcoded, u32, harmos_runtime::operations::Response<InputEnvelope<UploadDeclaration, u32>>>::new(
                operations.decode,
                operations.encode,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(*upload.into_inner(), 12);

    client.ready().await.unwrap();
    let mut download =
        client
            .server_streaming(
                tonic::Request::new(harmos_runtime::operations::Response::new(DownloadDeclaration { value: 5 })),
                tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Download"),
                Codec::<
                    harmos_runtime::grpc::Transcoded,
                    OutputEnvelope<u32, u32>,
                    harmos_runtime::operations::Response<DownloadDeclaration>,
                >::new(operations.decode, operations.encode)
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
            tonic::Request::new(opening(ExchangeDeclaration { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Exchange"),
            Codec::<
                harmos_runtime::grpc::Transcoded,
                OutputEnvelope<u32, u32>,
                harmos_runtime::operations::Response<InputEnvelope<ExchangeDeclaration, u32>>,
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
    assert_eq!(counts.started.load(Ordering::SeqCst), 3);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 3);
    assert_eq!(counts.finished.load(Ordering::SeqCst), 3);
    fixture.shutdown().await.unwrap();
    assert!(fixture.runtime.grpc().methods().is_empty());
    fixture.runtime.shutdown().await.unwrap();
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
    fixture.runtime.shutdown().await.unwrap();
}
