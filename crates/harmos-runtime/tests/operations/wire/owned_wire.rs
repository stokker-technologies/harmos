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
use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{
    Backoff, FailureKind, Handler, Jitter, OperationDeclaration, OperationInstanceId, OperationPolicy, RetryPolicy,
};
use harmos_runtime::{Message, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Resources {
    events: Mutex<Vec<&'static str>>,
    ids: Mutex<Vec<OperationInstanceId>>,
}

// The Carrier Must Not Require A Sync Handler Object.
struct Read {
    resources: Arc<Resources>,
    attempts: Cell<usize>,
}

#[harmos_runtime::message]
#[derive(Debug)]
struct Text {
    value: String,
}

/* === Enums === */

#[harmos_runtime::operation(response = Text,
        id = "owned_wire_read",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
            },
        },
    )]
struct ReadContractDeclaration {
    value: String,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContractDeclaration(ReadContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Handler for Read {
    type Request = ReadContractDeclaration;
    type Response = <ReadContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type StreamEndpoints = ();

    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Self::Error> {
        let resources = context.resource::<Resources>().expect("fixture registers its dependency");
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let constructed: Result<Self, Self::Error> = async move {
            resources.events.lock().unwrap().push("new");
            Ok(Self { resources, attempts: Cell::new(0) })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Self::Error> = async {
            let operation = &mut operation;
            operation.resources.events.lock().unwrap().push("initialize");
            operation.resources.ids.lock().unwrap().push(context.id());
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run(
        &mut self, context: harmos_runtime::ExecutionContext, request: &mut ReadContractDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Text>, Self::Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<Text, Self::Error> = async move {
            self.resources.events.lock().unwrap().push("run");
            self.resources.ids.lock().unwrap().push(context.id());
            assert_eq!(context.attempt(), self.attempts.get());
            self.attempts.set(self.attempts.get() + 1);
            Ok(Text {
                value: format!("{}:{}", request.value, self.attempts.get()),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.resources.events.lock().unwrap().push("stop");
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.events.lock().unwrap().push("finalize");
        Ok(())
    }

    fn classify(_: &Self::Error) -> FailureKind {
        FailureKind::Transient
    }
}

impl Drop for Read {
    fn drop(&mut self) {
        self.resources.events.lock().unwrap().push("drop");
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Resources>) {
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let resources = Arc::new(Resources::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(resources.clone()).unwrap();
                dependencies
            })
            .implementation::<ReadContractDeclaration, Read>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert!(resources.events.lock().unwrap().is_empty());
    (runtime, resources)
}

#[tokio::test]
async fn wire_bindings_execute() {
    {
        let resources = Arc::new(Resources::default());
        let runtime = Runtime::<App>::builder((), ())
            .register(Lanes::catalog())
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
                ..Default::default()
            })
            .with_registration(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(resources.clone()).unwrap();
                    dependencies
                })
                .implementation::<ReadContractDeclaration, Read>(),
                Lanes::Open,
            ))
            .runtime()
            .await
            .unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let invocation = runtime
            .operations
            .submit_encoded(&format!("host.{}", ReadContractDeclaration::ID), Text { value: "hello".into() }.encode())
            .unwrap();
        let response = invocation.await.unwrap();
        assert_eq!(Text::decode(&response).unwrap().value, "hello:1");
        assert_eq!(resources.events.lock().unwrap().iter().filter(|event| **event == "new").count(), 1);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, resources) = runtime().await;
        let encoded = runtime
            .operations
            .submit_encoded(&format!("host.{}", ReadContractDeclaration::ID), payload("complete"))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(Text::decode(&encoded).unwrap().value, "complete:1");
        assert_eq!(*resources.events.lock().unwrap(), ["new", "initialize", "run", "stop", "finalize", "drop"]);
        let answer = runtime.operation(ReadContractDeclaration { value: "local".to_owned() }).run().await.unwrap();
        assert_eq!(answer.value, "local:1");
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn registration_refuses_stream() {
    #[harmos_runtime::operation(
        response = (),
        input = Text,
        output = (),
        id = "owned_wire_stream",
        policy = ReadContractDeclaration::POLICY,
    )]
    struct StreamContract {}

    #[harmos_runtime::catalog]
    enum StreamContracts {
        StreamContract(StreamContract),
    }
    struct Stream;
    impl Handler for Stream {
        type Request = StreamContract;
        type Response = <StreamContract as harmos_runtime::OperationDeclaration>::Response;
        type Error = std::io::Error;
        type StreamEndpoints = ();
        async fn initialize(_: harmos_runtime::ExecutionContext) -> Result<Self, Self::Error> {
            panic!("unsupported binding must not construct an operation")
        }
        async fn run(
            &mut self, _: harmos_runtime::ExecutionContext, _request: &mut StreamContract, _endpoints: &mut Self::StreamEndpoints,
        ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
            let result: ::core::result::Result<(), Self::Error> = async move { unreachable!() }.await;
            result.map(::core::convert::Into::into)
        }
    }
    let (runtime, resources) = runtime().await;
    let contracts = harmos_runtime::operations::Contracts::of::<StreamContracts>().unwrap();
    assert!(contracts[0].input().is_some());
    assert!(contracts[0].output().unwrap().fields().is_empty());
    assert!(matches!(
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new()).implementation::<StreamContract, Stream>(),
                Lanes::Open
            ))
            .await,
        Err(harmos_runtime::Error::InvalidInput { field: "operation endpoint types", .. })
    ));
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn contracts_guard_identity() {
    let (runtime, resources) = runtime().await;
    for change_policy in [false, true] {
        let mut batch = harmos_runtime::RegistrationBatch::new({
            let mut dependencies = harmos_runtime::Dependencies::new();
            dependencies.insert(resources.clone()).unwrap();
            dependencies
        })
        .implementation::<ReadContractDeclaration, Read>();
        if change_policy {
            batch.operations[0].definition.policy.timeout += Duration::from_secs(1);
        } else {
            batch.operations[0].definition.id = harmos_runtime::Identifier::new("camera_read").unwrap();
        }
        assert!(runtime.operations.register(harmos_runtime::RegistrationExt::lane(batch, Lanes::Open)).await.is_err());
    }
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.stop().await.unwrap();
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn owner_assignments_isolate_instances() {
    let (runtime, _) = runtime().await;
    let mut owners = Vec::new();
    for owner in ["camera", "simulator"] {
        let resources = Arc::new(Resources::default());
        runtime
            .plugin
            .register_embedded(
                harmos_runtime::plugin::PluginDefinition {
                    id: owner.into(),
                    version: "1.0.0".parse().unwrap(),
                    artifact: harmos_runtime::plugin::PluginArtifact::Embedded { revision: "fixture".into() },
                    requires: vec![],
                    configuration: b"null".to_vec(),
                    contributions: Default::default(),
                },
                Default::default(),
                harmos_runtime::RegistrationExt::lane(
                    harmos_runtime::RegistrationBatch::new({
                        let mut dependencies = harmos_runtime::Dependencies::new();
                        dependencies.insert(resources.clone()).unwrap();
                        dependencies
                    })
                    .implementation::<ReadContractDeclaration, Read>(),
                    Lanes::Open,
                ),
            )
            .await
            .unwrap();
        runtime.plugin.initialize().await.unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let response = runtime
            .operations
            .submit_encoded(&format!("{owner}.{}", ReadContractDeclaration::ID), Text { value: owner.into() }.encode())
            .unwrap()
            .await
            .unwrap();
        assert_eq!(Text::decode(&response).unwrap().value, format!("{owner}:1"));
        assert_eq!(*resources.events.lock().unwrap(), ["new", "initialize", "run", "stop", "finalize", "drop"]);
        owners.push(resources);
    }
    assert_ne!(owners[0].ids.lock().unwrap()[0], owners[1].ids.lock().unwrap()[0]);
    runtime.stop().await.unwrap();
}

fn payload(value: &str) -> Vec<u8> {
    Text { value: value.to_owned() }.encode()
}

/* === Tests === */
