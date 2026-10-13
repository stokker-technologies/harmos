/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[cfg(feature = "grpc")]
use crate::scenario_runtime;

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
use harmos::grpc::{Codec, InputEnvelope, OutputEnvelope};
use harmos::operations::{
    Operation, StreamInput, StreamOutput, StreamSettings, WireStreamLimits, WireTable,
};

/* === Constants === */

/* === Macros === */
macro_rules! serving {
    ($owner:ident, $declaration:ident, $endpoints:ty, $request:ident, $ports:ident, $run:block) => {
        struct $owner(Arc<Counts>);
        impl Operation for $owner {
            type Declaration = $declaration;
            type Resources = Counts;
            type Error = harmos::Error;
            type Endpoints = $endpoints;
            type Context<'a> = &'a harmos::plugin::sidecar::Supervisor;
            fn new(counts: Arc<Counts>) -> Result<Self, Self::Error> {
                counts.prepared.fetch_add(1, Ordering::SeqCst);
                Ok(Self(counts))
            }
            async fn initialize<'a>(&mut self, _: Self::Context<'a>) -> Result<(), Self::Error> {
                self.0.operation_initialized.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            async fn run<'a>(
                &mut self,
                _: Self::Context<'a>,
                $request: &mut u32,
                $ports: &mut Self::Endpoints,
            ) -> Result<harmos::operations::Response<u32>, Self::Error> {
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
            async fn finalize(&mut self) -> Result<(), Self::Error> {
                self.0.finalized.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::operation(request = u32, response = u32, input = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Native", method = "Upload"))]
pub(super) struct Upload;

#[harmos::operation(request = u32, response = u32, output = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Native", method = "Download"))]
pub(super) struct Download;

#[harmos::operation(request = u32, response = u32, input = u32, output = u32, policy = Read::POLICY, version = 1,
        grpc(package = "fixture.v1", service = "Native", method = "Exchange"))]
pub(super) struct Exchange;

#[harmos::catalog]
pub(super) enum Operations {
    Upload(Upload),
    Download(Download),
    Exchange(Exchange),
}

// Runtime-error endpoints bind through the same catalog projection as
// `bind_operations`, which is the authored sidecar route.
#[harmos::catalog]
enum Implemented {
    Upload(Uploading),
    Download(Downloading),
    Exchange(Exchanging),
}

/* === Traits === */

/* === Implementations === */
serving!(Uploading, Upload, StreamInput<u32>, request, input, {
    let mut total = *request;
    while let Some(item) = input.recv().await {
        total += *item;
    }
    Ok(total)
});
serving!(Downloading, Download, StreamOutput<u32>, request, output, {
    for value in [0, *request] {
        output.send_with(4, || Ok(value)).await?;
    }
    Ok(*request)
});
serving!(
    Exchanging,
    Exchange,
    (StreamInput<u32>, StreamOutput<u32>),
    request,
    ports,
    {
        let mut total = *request;
        while let Some(item) = ports.0.recv().await {
            total += *item;
            ports.1.send_with(4, || Ok(total)).await?;
        }
        Ok(total)
    }
);

/* === Public Functions === */
pub(super) fn bind(
    table: &mut WireTable<harmos::plugin::sidecar::Supervisor>,
    counts: Arc<Counts>,
) -> Result<(), harmos::operations::ServingError> {
    let direction = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(64).unwrap(),
    };
    let limits = WireStreamLimits {
        input: direction,
        output: direction,
    };
    table.bind_streaming_operations::<Implemented, _>(counts, limits)
}

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn opening()
-> impl futures_core::Stream<Item = harmos::operations::Response<InputEnvelope<u32, u32>>>
+ Send
+ 'static {
    futures_util::stream::iter([
        harmos::operations::Response::new(InputEnvelope::Request(5)),
        harmos::operations::Response::new(InputEnvelope::Item(0)),
        harmos::operations::Response::new(InputEnvelope::Item(7)),
    ])
}

/* === Tests === */

#[test]
fn contract_scenario() {
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("grpc_stream_lifecycle", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    assert_eq!(fixture.runtime.grpc().methods().len(), 4);
                    let operations = harmos::grpc::server::Settings::default();
                    let attached = fixture
                        .runtime
                        .grpc()
                        .attach(harmos::grpc::Settings {
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
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    let mut client = tonic::client::Grpc::new(channel);
                    client.ready().await.unwrap();
                    let upload: tonic::Response<harmos::operations::StreamItem<u32>> = client
                        .client_streaming(
                            tonic::Request::new(opening()),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.v1.Native/Upload",
                            ),
                            Codec::<u32, harmos::operations::Response<InputEnvelope<u32, u32>>>::new(
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
                            tonic::Request::new(harmos::operations::Response::new(5u32)),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.v1.Native/Download",
                            ),
                            Codec::<OutputEnvelope<u32, u32>, harmos::operations::Response<u32>>::new(
                                operations.decode,
                                operations.encode,
                            )
                            .unwrap(),
                        )
                        .await
                        .unwrap()
                        .into_inner();
                    for expected in [
                        OutputEnvelope::Item(0),
                        OutputEnvelope::Item(5),
                        OutputEnvelope::Response(5),
                    ] {
                        assert_eq!(*download.message().await.unwrap().unwrap(), expected);
                    }
                    assert!(download.message().await.unwrap().is_none());

                    client.ready().await.unwrap();
                    let mut exchange = client
                        .streaming(
                            tonic::Request::new(opening()),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.v1.Native/Exchange",
                            ),
                            Codec::<OutputEnvelope<u32, u32>, harmos::operations::Response<InputEnvelope<u32, u32>>>::new(
                                operations.decode,
                                operations.encode,
                            )
                            .unwrap(),
                        )
                        .await
                        .unwrap()
                        .into_inner();
                    for expected in [
                        OutputEnvelope::Item(5),
                        OutputEnvelope::Item(12),
                        OutputEnvelope::Response(12),
                    ] {
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
                });
        });
    }
}
