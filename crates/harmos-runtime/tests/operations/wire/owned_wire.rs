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
    Backoff, Context, FailureKind, Jitter, Operation, OperationDeclaration, OperationInstanceId, OperationPolicy, RestartPolicy,
    RetryPolicy,
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

// The Carrier Must Not Require A Sync Operation Object.
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
#[harmos_runtime::message]
#[harmos_runtime::operation(
        response = Text,
        id = "owned-wire.read",
        version = 1,
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
            },
            restart: RestartPolicy::Never,
        },
    )]
struct ReadContract {
    value: String,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContract(ReadContract),
}

#[harmos_runtime::catalog]
enum Authored {
    #[nested]
    Inner(Implementation),
    #[nested]
    Empty(()),
}

#[harmos_runtime::catalog]
enum Implementation {
    Read(Read),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Read {
    type Declaration = ReadContract;
    type Resources = Resources;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(resources: Arc<Resources>, context: Self::Context<'ctx>) -> Result<Self, Self::Error> {
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

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut ReadContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Text>, Self::Error> {
        let result: ::core::result::Result<Text, Self::Error> = async move {
            self.resources.events.lock().unwrap().push("run");
            self.resources.ids.lock().unwrap().push(context.id());
            assert_eq!(context.attempt(), self.attempts.get());
            self.attempts.set(self.attempts.get() + 1);
            Ok(Text { value: format!("{}:{}", request.value, self.attempts.get()) })
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
    runtime.operations.register_operations::<Authored, _>("app", Lanes::Open, resources.clone()).unwrap();
    assert!(resources.events.lock().unwrap().is_empty());
    (runtime, resources)
}

#[tokio::test]
async fn builder_wire_catalog() {
    let resources = Arc::new(Resources::default());
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        })
        .wire_operations::<Authored, _>("app", Lanes::Open, Arc::clone(&resources))
        .runtime()
        .await
        .unwrap();
    assert!(resources.events.lock().unwrap().is_empty());
    let invocation = runtime.operations.submit_encoded(ReadContract::ID, Text { value: "hello".into() }.encode_to_vec()).unwrap();
    let response = invocation.await.unwrap();
    assert_eq!(Text::decode(&response).unwrap().value, "hello:1");
    assert_eq!(resources.events.lock().unwrap().iter().filter(|event| **event == "new").count(), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn registration_projects_wire() {
    let (runtime, resources) = runtime().await;
    let encoded = runtime.operations.submit_encoded(ReadContract::ID, payload("complete")).unwrap().await.unwrap();
    assert_eq!(Text::decode(&encoded).unwrap().value, "complete:1");
    assert_eq!(*resources.events.lock().unwrap(), ["new", "initialize", "run", "stop", "finalize", "drop"]);
    let answer = runtime.operations.call(ReadContract { value: "local".to_owned() }).await.unwrap();
    assert_eq!(answer.value, "local:1");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn registration_refuses_stream() {
    #[harmos_runtime::message]
    #[harmos_runtime::operation(response = (), input = Text, output = (), id = "owned-wire.stream", version = 1, policy = ReadContract::POLICY)]
    struct StreamContract {}

    #[harmos_runtime::catalog]
    enum StreamContracts {
        StreamContract(StreamContract),
    }
    struct Stream;
    impl Operation for Stream {
        type Declaration = StreamContract;
        type Resources = Resources;
        type Error = std::io::Error;
        type Endpoints = ();
        type Context<'a> = Context<'a>;
        async fn initialize<'ctx>(_: Arc<Resources>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
            panic!("unsupported binding must not construct an operation")
        }
        async fn run<'ctx>(
            &mut self, _: Self::Context<'ctx>, _request: &mut StreamContract, _endpoints: &mut Self::Endpoints,
        ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
            let result: ::core::result::Result<(), Self::Error> = async move { unreachable!() }.await;
            result.map(::core::convert::Into::into)
        }
    }
    #[harmos_runtime::catalog]
    enum Implemented {
        Stream(Stream),
    }
    let (runtime, resources) = runtime().await;
    let contracts = harmos_runtime::operations::Contracts::of::<StreamContracts>().unwrap();
    assert!(contracts[0].input().is_some());
    assert!(contracts[0].output().unwrap().fields().is_empty());
    assert!(matches!(
        runtime.operations.register_wire_operations::<Implemented, _>("app", Lanes::Open, resources.clone()),
        Err(harmos_runtime::Error::InvalidInput { field: "operation endpoint types", .. })
    ));
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn contracts_retain_distinct() {
    use harmos_runtime::operations::{Contract, WireDeclaration};
    let (runtime, _) = runtime().await;
    let declaration = WireDeclaration::of::<ReadContract>().contract().unwrap();
    let mut owners = Vec::new();
    for owner in ["camera", "simulator"] {
        let resources = Arc::new(Resources::default());
        let mut definition = declaration.operation().clone();
        definition.id = harmos_runtime::Identifier::new(format!("{owner}.read")).unwrap();
        let contract = Contract::new(definition, declaration.version(), declaration.codec(), declaration.schema().clone()).unwrap();
        runtime.operations.register_wire_contract::<Read>(owner, &contract, Lanes::Open, resources.clone()).unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let response = runtime
            .operations
            .submit_encoded(contract.operation().id.as_str(), Text { value: owner.into() }.encode_to_vec())
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

#[tokio::test]
async fn contract_rejects_drift() {
    use harmos_runtime::operations::{Contract, WireDeclaration};
    let (runtime, resources) = runtime().await;
    let declaration = WireDeclaration::of::<ReadContract>().contract().unwrap();
    for change_policy in [false, true] {
        let mut definition = declaration.operation().clone();
        definition.id = harmos_runtime::Identifier::new("camera.read").unwrap();
        if change_policy {
            definition.policy.timeout += Duration::from_secs(1);
        }
        let contract =
            Contract::new(definition, declaration.version() + u32::from(!change_policy), declaration.codec(), declaration.schema().clone())
                .unwrap();
        assert!(runtime.operations.register_wire_contract::<Read>("camera", &contract, Lanes::Open, resources.clone()).is_err());
    }
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.stop().await.unwrap();
}

fn payload(value: &str) -> Vec<u8> {
    Text { value: value.to_owned() }.encode_to_vec()
}

/* === Tests === */
