/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod storage;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports

// Local Imports
use harmos::operations::{Context, Operation, StreamInput, StreamOutput};
use harmos::{Application, Runtime, state::Never};
use harmos_grpc_fixture::contract::*;

/* === Constants === */

/* === Macros === */
macro_rules! operation {
    ($name:ident, $declaration:ty, $endpoints:ty, $this:ident, $ports:ident, $body:block) => {
        struct $name {
            trace: Arc<Trace>,
        }
        impl Operation for $name {
            type Declaration = $declaration;
            type Resources = Trace;
            type Error = harmos::Error;
            type Context<'a> = Context<'a>;
            type Endpoints = $endpoints;
            fn new(trace: Arc<Trace>) -> harmos::Result<Self> {
                trace.new.fetch_add(1, Ordering::SeqCst);
                trace.started_event.notify_one();
                Ok(Self { trace })
            }
            async fn run<'a>(
                &mut self,
                _: Self::Context<'a>,
                request: &mut Payload,
                endpoints: &mut Self::Endpoints,
            ) -> harmos::Result<Payload> {
                let $this = request;
                let $ports = endpoints;
                $body
            }
            async fn finalize(&mut self) -> harmos::Result<()> {
                self.trace.finalized.fetch_add(1, Ordering::SeqCst);
                if self.trace.fail_finalize.load(Ordering::SeqCst) {
                    return Err(harmos::Error::InvalidInput {
                        field: "fixture cleanup",
                        reason: "scripted finalization failure",
                    });
                }
                Ok(())
            }
        }
        impl Drop for $name {
            fn drop(&mut self) {
                self.trace.dropped.fetch_add(1, Ordering::SeqCst);
                self.trace.dropped_event.notify_one();
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */
struct App;
#[derive(Default)]
struct Trace {
    fail_finalize: AtomicBool,
    started_event: tokio::sync::Notify,
    dropped_event: tokio::sync::Notify,
    new: AtomicUsize,
    finalized: AtomicUsize,
    dropped: AtomicUsize,
}

struct Network {
    runtime: Runtime<App>,
    trace: Arc<Trace>,
    client: harmos_grpc_fixture::fixture::v1::operations_client::OperationsClient<
        tonic::transport::Channel,
    >,
    stop: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

/* === Enums === */
#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */
impl Network {
    async fn new(
        opening_timeout: Duration,
        calls: usize,
    ) -> Result<Self, harmos_grpc_fixture::Error> {
        let runtime = Runtime::<App>::builder((), ())
            .register(Lanes::catalog())
            .runtime()
            .await
            .expect("valid fixture catalog");
        let trace = Arc::new(Trace::default());
        runtime
            .operations
            .register_operation::<UnaryCall>("fixture", Lanes::Open, trace.clone())
            .unwrap();
        runtime
            .operations
            .register_operation::<UploadCall>("fixture", Lanes::Open, trace.clone())
            .unwrap();
        runtime
            .operations
            .register_operation::<DownloadCall>("fixture", Lanes::Open, trace.clone())
            .unwrap();
        runtime
            .operations
            .register_operation::<ExchangeCall>("fixture", Lanes::Open, trace.clone())
            .unwrap();
        let capacity = harmos::operations::StreamSettings {
            items: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(1024).unwrap(),
        };
        let settings = harmos::grpc::server::Settings {
            calls: NonZeroUsize::new(calls).unwrap(),
            decode: harmos::grpc::DecodeSettings {
                message_bytes: NonZeroUsize::new(1024).unwrap(),
                retained_bytes: NonZeroUsize::new(4096).unwrap(),
            },
            encode: harmos::grpc::EncodeSettings {
                message_bytes: NonZeroUsize::new(1024).unwrap(),
                buffer_bytes: NonZeroUsize::new(64).unwrap(),
            },
            input: capacity,
            output: capacity,
            opening_timeout,
        };
        let invalid = harmos::grpc::server::Settings {
            opening_timeout: Duration::ZERO,
            ..settings
        };
        let refused = harmos_grpc_fixture::OperationsServer::new(
            harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
            invalid,
        );
        assert!(matches!(refused, Err(error) if error.code() == tonic::Code::InvalidArgument));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let incoming = futures_util::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(socket, _)| socket), listener))
        });
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let service = harmos_grpc_fixture::OperationsServer::new(
            harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
            settings,
        )
        .expect("valid transport settings");
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                }),
        );
        let client =
            harmos_grpc_fixture::fixture::v1::operations_client::OperationsClient::connect(
                format!("http://{address}"),
            )
            .await?;
        Ok(Self {
            runtime,
            trace,
            client,
            stop,
            server,
        })
    }
}

impl Application for App {
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}
operation!(UnaryCall, Unary, (), this, _ports, {
    Ok(Payload {
        value: format!("unary:{}", this.value),
    })
});
operation!(
    DownloadCall,
    Download,
    StreamOutput<Payload>,
    this,
    ports,
    {
        ports
            .send_with(128, || {
                Ok(Payload {
                    value: format!("item:{}", this.value),
                })
            })
            .await?;
        Ok(Payload {
            value: "download:done".into(),
        })
    }
);
operation!(UploadCall, Upload, StreamInput<Payload>, this, ports, {
    let mut value = this.value.clone();
    while let Some(item) = ports.recv().await {
        value.push_str(&item.value);
    }
    Ok(Payload { value })
});
operation!(
    ExchangeCall,
    Exchange,
    (StreamInput<Payload>, StreamOutput<Payload>),
    this,
    ports,
    {
        if this.value == "independent" || this.value == "partial-failure" {
            for value in 0..4 {
                ports
                    .1
                    .send_with(128, || {
                        Ok(Payload {
                            value: format!("burst:{value}"),
                        })
                    })
                    .await?;
            }
            if this.value == "partial-failure" {
                return Err(harmos::Error::InvalidInput {
                    field: "fixture output",
                    reason: "failure after transferred items",
                });
            }
        }
        while let Some(item) = ports.0.recv().await {
            ports
                .1
                .send_with(128, || {
                    Ok(Payload {
                        value: format!("{}:{}", this.value, item.value),
                    })
                })
                .await?;
        }
        if this.value == "independent" {
            ports
                .1
                .send_with(128, || {
                    Ok(Payload {
                        value: "after-eof".into(),
                    })
                })
                .await?;
        }
        Ok(Payload {
            value: "exchange:done".into(),
        })
    }
);

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[tokio::test]
async fn stock_generated_client() -> Result<(), harmos_grpc_fixture::Error> {
    tokio::time::timeout(Duration::from_secs(5), async {
        use harmos_grpc_fixture::fixture::v1::*;
        use operations_messages as messages;
        let Network { runtime, trace, mut client, stop, server } = Network::new(Duration::from_secs(1), 8).await?;
        // Reject Invalid Openings Before Construction
        let missing = client.upload(futures_util::stream::empty::<OperationsUploadInput>()).await.unwrap_err();
        assert_eq!(missing.code(), tonic::Code::InvalidArgument);
        let early = OperationsUploadInput {
            value: Some(operations_upload_input::Value::Item(messages::upload::Row1 { value: "early".into() })),
        };
        let invalid = client.upload(futures_util::stream::iter([early])).await.unwrap_err();
        assert_eq!(invalid.code(), tonic::Code::InvalidArgument);
        assert_eq!(trace.new.load(Ordering::SeqCst), 0);

        let response = client.unary(messages::unary::Row1 { value: "one".into() }).await.unwrap().into_inner();
        assert_eq!(response.value,"unary:one");
        let frames = [
            OperationsUploadInput { value: Some(operations_upload_input::Value::Request(messages::upload::Row1 { value: "start".into() })) },
            OperationsUploadInput { value: Some(operations_upload_input::Value::Item(messages::upload::Row1 { value: ":item".into() })) },
        ];
        let response = client.upload(futures_util::stream::iter(frames)).await.unwrap().into_inner();
        assert_eq!(response.value,"start:item");
        let mut output = client.download(messages::download::Row1 { value: "two".into() }).await.unwrap().into_inner();
        assert!(matches!(output.message().await.unwrap().unwrap().value, Some(operations_download_output::Value::Item(item)) if item.value == "item:two"));
        assert!(matches!(output.message().await.unwrap().unwrap().value, Some(operations_download_output::Value::Response(response)) if response.value == "download:done"));
        assert!(output.message().await.unwrap().is_none());
        let frames = [
            OperationsExchangeInput { value: Some(operations_exchange_input::Value::Request(messages::exchange::Row1 { value: "bidi".into() })) },
            OperationsExchangeInput { value: Some(operations_exchange_input::Value::Item(messages::exchange::Row1 { value: "item".into() })) },
        ];
        let mut output = client.exchange(futures_util::stream::iter(frames)).await.unwrap().into_inner();
        assert!(matches!(output.message().await.unwrap().unwrap().value, Some(operations_exchange_output::Value::Item(item)) if item.value == "bidi:item"));
        assert!(matches!(output.message().await.unwrap().unwrap().value, Some(operations_exchange_output::Value::Response(response)) if response.value == "exchange:done"));
        assert!(output.message().await.unwrap().is_none());
        drop(output); drop(client);
        stop.send(()).unwrap(); server.await.unwrap().unwrap();
        runtime.stop().await?;
        assert_eq!(trace.new.load(Ordering::SeqCst),4);
        assert_eq!(trace.finalized.load(Ordering::SeqCst),4);
        assert_eq!(trace.dropped.load(Ordering::SeqCst),4);
        Ok(())
    }).await.unwrap()
}

#[tokio::test]
async fn streams_settle_owner() -> Result<(), harmos_grpc_fixture::Error> {
    use futures_util::StreamExt as _;
    use harmos_grpc_fixture::fixture::v1::*;
    use operations_messages as messages;
    tokio::time::timeout(Duration::from_secs(5), async {
        let Network {
            runtime,
            trace,
            mut client,
            stop,
            server,
        } = Network::new(Duration::from_millis(50), 8).await?;

        // A Peer That Never Opens Must Not Construct An Operation
        let stalled = tokio::time::timeout(
            Duration::from_secs(1),
            client.upload(futures_util::stream::pending::<OperationsUploadInput>()),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(stalled.code(), tonic::Code::DeadlineExceeded);
        assert_eq!(trace.new.load(Ordering::SeqCst), 0);

        // Duplicate Openings Must Settle The One Admitted Operation
        let opening = || OperationsUploadInput {
            value: Some(operations_upload_input::Value::Request(
                messages::upload::Row1 {
                    value: "duplicate".into(),
                },
            )),
        };
        let duplicate = client
            .upload(futures_util::stream::iter([opening(), opening()]))
            .await
            .unwrap_err();
        assert_eq!(duplicate.code(), tonic::Code::InvalidArgument);
        assert_eq!(trace.new.load(Ordering::SeqCst), 1);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);

        // Aborting The Whole RPC Resets Its Still-Open Request Stream
        let opening = OperationsUploadInput {
            value: Some(operations_upload_input::Value::Request(
                messages::upload::Row1 {
                    value: "abandoned".into(),
                },
            )),
        };
        let incoming = futures_util::stream::iter([opening]).chain(futures_util::stream::pending());
        let mut abandoned_client = client.clone();
        let abandoned = tokio::spawn(async move { abandoned_client.upload(incoming).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while trace.new.load(Ordering::SeqCst) != 2 {
                trace.started_event.notified().await;
            }
        })
        .await
        .unwrap();
        abandoned.abort();
        assert!(abandoned.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_millis(500), async {
            while trace.dropped.load(Ordering::SeqCst) != 2 {
                trace.dropped_event.notified().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 2);

        drop(client);
        stop.send(()).unwrap();
        server.await.unwrap()?;
        runtime.stop().await?;
        Ok(())
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn call_limit_refuses() -> Result<(), harmos_grpc_fixture::Error> {
    use futures_util::StreamExt as _;
    use harmos_grpc_fixture::fixture::v1::*;
    use operations_messages as messages;
    tokio::time::timeout(Duration::from_secs(5), async {
        let Network {
            runtime,
            trace,
            mut client,
            stop,
            server,
        } = Network::new(Duration::from_secs(1), 1).await?;
        let opening = OperationsUploadInput {
            value: Some(operations_upload_input::Value::Request(
                messages::upload::Row1 {
                    value: "held".into(),
                },
            )),
        };
        let incoming = futures_util::stream::iter([opening]).chain(futures_util::stream::pending());
        let mut first_client = client.clone();
        let first = tokio::spawn(async move { first_client.upload(incoming).await });
        while trace.new.load(Ordering::SeqCst) != 1 {
            trace.started_event.notified().await;
        }
        let refused = client
            .unary(messages::unary::Row1 {
                value: "excess".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(refused.code(), tonic::Code::ResourceExhausted);
        assert_eq!(trace.new.load(Ordering::SeqCst), 1);
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_millis(500), async {
            while trace.dropped.load(Ordering::SeqCst) != 1 {
                trace.dropped_event.notified().await;
            }
        })
        .await
        .unwrap();
        let response = client
            .unary(messages::unary::Row1 {
                value: "released".into(),
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.value, "unary:released");
        drop(client);
        stop.send(()).unwrap();
        server.await.unwrap()?;
        runtime.stop().await?;
        assert_eq!(trace.new.load(Ordering::SeqCst), 2);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 2);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
        Ok(())
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn bidi_outputs_progress() -> Result<(), harmos_grpc_fixture::Error> {
    use harmos_grpc_fixture::fixture::v1::*;
    use operations_messages as messages;
    tokio::time::timeout(Duration::from_secs(5), async {
        let Network { runtime, trace, mut client, stop, server } = Network::new(Duration::from_secs(1), 1).await?;
        let (input, receiving) = tokio::sync::mpsc::channel(1);
        let opening = OperationsExchangeInput { value: Some(operations_exchange_input::Value::Request(messages::exchange::Row1 { value: "live".into() })) };
        input.send(opening).await.unwrap();
        let incoming = futures_util::stream::unfold(receiving, |mut receiving| async move {
            receiving.recv().await.map(|item| (item, receiving))
        });
        let mut output = client.exchange(incoming).await.unwrap().into_inner();
        for value in ["one", "two", "three"] {
            let item = OperationsExchangeInput { value: Some(operations_exchange_input::Value::Item(messages::exchange::Row1 { value: value.into() })) };
            input.send(item).await.unwrap();
            let event = tokio::time::timeout(Duration::from_millis(500), output.message()).await.unwrap().unwrap().unwrap();
            assert!(matches!(event.value, Some(operations_exchange_output::Value::Item(item)) if item.value == format!("live:{value}")));
            assert_eq!(trace.finalized.load(Ordering::SeqCst), 0);
        }
        drop(input);
        let completed = output.message().await.unwrap().unwrap();
        assert!(matches!(completed.value, Some(operations_exchange_output::Value::Response(response)) if response.value == "exchange:done"));
        assert!(output.message().await.unwrap().is_none());
        drop(output);
        drop(client);
        stop.send(()).unwrap();
        server.await.unwrap()?;
        runtime.stop().await?;
        assert_eq!(trace.new.load(Ordering::SeqCst), 1);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
        Ok(())
    }).await.unwrap()
}

#[tokio::test]
async fn failure_retains_owner() -> Result<(), harmos_grpc_fixture::Error> {
    use harmos_grpc_fixture::fixture::v1::operations_messages as messages;
    tokio::time::timeout(Duration::from_secs(5), async {
        let Network {
            runtime,
            trace,
            mut client,
            stop,
            server,
        } = Network::new(Duration::from_secs(1), 1).await?;
        trace.fail_finalize.store(true, Ordering::SeqCst);
        let error = client
            .unary(messages::unary::Row1 {
                value: "retained".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::Internal);
        assert_eq!(trace.new.load(Ordering::SeqCst), 1);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
        drop(client);
        stop.send(()).unwrap();
        server.await.unwrap()?;
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
        trace.fail_finalize.store(false, Ordering::SeqCst);
        runtime.stop().await?;
        assert_eq!(trace.new.load(Ordering::SeqCst), 1);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 2);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
        Ok(())
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn failure_preserve_stream() -> Result<(), harmos_grpc_fixture::Error> {
    use harmos_grpc_fixture::fixture::v1::*;
    use operations_messages as messages;
    tokio::time::timeout(Duration::from_secs(5), async {
        let Network { runtime, trace, mut client, stop, server } = Network::new(Duration::from_secs(1), 1).await?;
        for mode in ["independent", "partial-failure"] {
            let (input, receiving) = tokio::sync::mpsc::channel(1);
            input.send(OperationsExchangeInput {
                value: Some(operations_exchange_input::Value::Request(messages::exchange::Row1 { value: mode.into() })),
            }).await.unwrap();
            let incoming = futures_util::stream::unfold(receiving, |mut receiving| async move {
                receiving.recv().await.map(|item| (item, receiving))
            });
            let mut output = client.exchange(incoming).await.unwrap().into_inner();
            // No Input Item Has Been Sent; A Request/Reply-Only Adapter Cannot Pass.
            for value in 0..4 {
                let event = output.message().await.unwrap().unwrap();
                assert!(matches!(event.value, Some(operations_exchange_output::Value::Item(item)) if item.value == format!("burst:{value}")));
            }
            if mode == "partial-failure" {
                assert_eq!(output.message().await.unwrap_err().code(), tonic::Code::Internal);
                drop(input);
                continue;
            }
            let sending = async move {
                for value in 0..128 {
                    input.send(OperationsExchangeInput {
                        value: Some(operations_exchange_input::Value::Item(messages::exchange::Row1 { value: value.to_string() })),
                    }).await.unwrap();
                }
                // Dropping The Complete Request Producer Half-Closes HTTP/2 Input.
            };
            let receiving = async {
                for value in 0..128 {
                    let event = output.message().await.unwrap().unwrap();
                    assert!(matches!(event.value, Some(operations_exchange_output::Value::Item(item)) if item.value == format!("independent:{value}")));
                }
                let tail = output.message().await.unwrap().unwrap();
                assert!(matches!(tail.value, Some(operations_exchange_output::Value::Item(item)) if item.value == "after-eof"));
                let completed = output.message().await.unwrap().unwrap();
                assert!(matches!(completed.value, Some(operations_exchange_output::Value::Response(response)) if response.value == "exchange:done"));
                assert!(output.message().await.unwrap().is_none());
            };
            tokio::join!(sending, receiving);
        }
        drop(client);
        stop.send(()).unwrap();
        server.await.unwrap()?;
        runtime.stop().await?;
        assert_eq!(trace.new.load(Ordering::SeqCst), 2);
        assert_eq!(trace.finalized.load(Ordering::SeqCst), 2);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
        Ok(())
    }).await.unwrap()
}
