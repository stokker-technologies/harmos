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
use super::{Counts, Read};
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
            type Declaration = $declaration;
            type Resources = Counts;
            type Error = harmos_runtime::Error;
            type Endpoints = $endpoints;
            type Context<'a> = &'a harmos_runtime::sidecar::Supervisor;
            async fn initialize<'a>(counts: Arc<Counts>, _: Self::Context<'a>) -> Result<Self, Self::Error> {
                counts.prepared.fetch_add(1, Ordering::SeqCst);
                counts.operation_initialized.fetch_add(1, Ordering::SeqCst);
                Ok(Self(counts))
            }
            async fn run<'a>(
                &mut self, _: Self::Context<'a>, $request: &mut $declaration, $ports: &mut Self::Endpoints,
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
#[harmos_runtime::message]
#[harmos_runtime::operation(
    version = 1,
    policy = Read::POLICY,
    response = u32,
    input = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Upload"),
)]
pub(super) struct Upload {
    pub(super) value: u32,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    version = 1,
    policy = Read::POLICY,
    response = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Download"),
)]
pub(super) struct Download {
    pub(super) value: u32,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    version = 1,
    policy = Read::POLICY,
    response = u32,
    input = u32,
    output = u32,
    grpc(package = "fixture.v1", service = "Native", method = "Exchange"),
)]
pub(super) struct Exchange {
    pub(super) value: u32,
}

#[harmos_runtime::catalog]
pub(super) enum Operations {
    Upload(Upload),
    Download(Download),
    Exchange(Exchange),
}

// Runtime-error endpoints bind through the same catalog projection as
// `bind_operations`, which is the authored sidecar route.
#[harmos_runtime::catalog]
enum Implemented {
    Upload(Uploading),
    Download(Downloading),
    Exchange(Exchanging),
}

/* === Traits === */

/* === Implementations === */
serving!(Uploading, Upload, StreamInput<u32>, request, input, {
    let mut total = request.value;
    while let Some(item) = input.recv().await {
        total += *item;
    }
    Ok(total)
});
serving!(Downloading, Download, StreamOutput<u32>, request, output, {
    for value in [0, request.value] {
        output.send_with(4, || Ok(value)).await?;
    }
    Ok(request.value)
});
serving!(Exchanging, Exchange, (StreamInput<u32>, StreamOutput<u32>), request, ports, {
    let mut total = request.value;
    while let Some(item) = ports.0.recv().await {
        total += *item;
        ports.1.send_with(4, || Ok(total)).await?;
    }
    Ok(total)
});

/* === Public Functions === */
pub(super) fn bind(
    table: &mut WireTable<harmos_runtime::sidecar::Supervisor>, counts: Arc<Counts>,
) -> Result<(), harmos_runtime::operations::ServingError> {
    let direction = StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(64).unwrap() };
    let limits = WireStreamLimits { input: direction, output: direction };
    table.bind_streaming_operations::<Implemented, _>(counts, limits)
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
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
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
            Codec::<harmos_runtime::grpc::Transcoded, u32, harmos_runtime::operations::Response<InputEnvelope<Upload, u32>>>::new(operations.decode, operations.encode).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(*upload.into_inner(), 12);

    client.ready().await.unwrap();
    let mut download = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new(Download { value: 5 })),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<Download>>::new(operations.decode, operations.encode).unwrap(),
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
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<InputEnvelope<Exchange, u32>>>::new(
                operations.decode,
                operations.encode,
            )
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
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
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

    // Field 1 Carries A String Where The Declaration Expects A u32.
    let refused = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new("not a number".to_owned())),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<String>>::new(operations.decode, operations.encode).unwrap(),
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
