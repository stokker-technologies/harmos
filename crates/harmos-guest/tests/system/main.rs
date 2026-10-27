/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod plugin;
#[path = "../../../harmos-runtime/tests/support/report.rs"]
mod report;

#[path = "../fixtures/sandbox/src/contract.rs"]
mod sandbox;

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::VecDeque;
use std::sync::OnceLock;
// Third Party Imports
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};
// Local Imports
use harmos::guest::capabilities::Refusal as BridgeRefusal;
use harmos_domain::inventory::{Declaration, Facility};
use harmos_domain::{Message, Refusal};
use harmos_operations::lifecycle::InitializeRequest;
use harmos_operations::lifecycle::InstanceState;
use harmos_sdk::operations::{WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCommand};
use sandbox::{Echo, Echoed};

/* === Constants === */

/* === Macros === */

wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
    exports: { default: async },
});

/* === Type Aliases === */

/* === Structs === */

// Records Every Host Call A Guest Makes And Answers The Provider From A Script
#[derive(Default)]
struct Stub {
    calls: Vec<Call>,
    provider: VecDeque<Result<Echoed, Refusal>>,
    // Submitted Children By Instance: What Each Asked For And Whether It Was Cancelled
    submitted: std::collections::BTreeMap<u64, (String, bool)>,
}

struct Session {
    store: Store<Stub>,
    bindings: Plugin,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Provider { say: String },
    Publish { stream: String, payload: Vec<u8> },
    Other { operation: String },
    Submit { say: String, instance: u64 },
    Control { instance: u64, action: WireChildAction },
}

/* === Traits === */

/* === Implementations === */

impl harmos::guest::capabilities::Host for Stub {}

impl harmos::guest::operation_context::Host for Stub {}

impl<T: Send> harmos::guest::operation_context::HostWithStore<T> for HasSelf<Stub> {
    async fn child(accessor: &wasmtime::component::Accessor<T, Self>, payload: Vec<u8>) -> Result<Vec<u8>, BridgeRefusal> {
        let answer = accessor.with(|mut access| {
            let host = access.get();
            host.child(payload)
        });
        answer.map_err(|error| {
            let code = error.code().to_string();
            let failure = error.failure().encode();
            BridgeRefusal { code, failure }
        })
    }
}

impl Stub {
    // Scripted Transport Answers Prove Encoding, Not Root Authorization
    fn child(&mut self, payload: Vec<u8>) -> Result<Vec<u8>, Refusal> {
        let call = match WireChild::decode(&payload).map_err(Refusal::of)? {
            WireChild::Call(call) if call.submit => return self.submit(call),
            WireChild::Call(call) => call,
            WireChild::Control(control) => return self.control(control),
        };
        assert_ne!(call.context, [0; 32], "the SDK forwards the current finite calling context");
        match call.operation.as_str() {
            "provider.provider_echo" => {
                let request = Echo::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Provider { say: request.say.clone() });
                let fallback = Ok(Echoed { said: request.say });
                let answer = self.provider.pop_front().unwrap_or(fallback)?;
                Ok(answer.encode())
            }
            "host.host_publish" => {
                let request = harmos_operations::streams::PublishRequest::decode(&call.payload).map_err(Refusal::of)?;
                assert_eq!(request.schema, 1);
                assert_eq!(request.sequence, None, "the guest asks its original lease to derive the sequence");
                assert_eq!(request.published_unix_micros, None);
                assert_eq!(request.context, None);
                assert!(request.following.is_empty());
                self.calls.push(Call::Publish { stream: request.stream, payload: request.payload });
                Ok(().encode())
            }
            operation => {
                self.calls.push(Call::Other { operation: operation.to_owned() });
                Err(Refusal::new(-32601, "no operation scope"))
            }
        }
    }
}

impl Stub {
    fn submit(&mut self, call: WireChildCall) -> Result<Vec<u8>, Refusal> {
        assert_eq!(call.operation, "provider.provider_echo");
        let request = Echo::decode(&call.payload).map_err(Refusal::of)?;
        let instance = self.submitted.len() as u64 + 1;
        self.submitted.insert(instance, (request.say.clone(), false));
        self.calls.push(Call::Submit { say: request.say, instance });
        Ok(instance.encode())
    }

    fn control(&mut self, control: WireChildControl) -> Result<Vec<u8>, Refusal> {
        assert_ne!(control.context, [0; 32], "controls name the attempt");
        let instance = control.instance;
        self.calls.push(Call::Control { instance, action: control.action });
        let submitted = self.submitted.get(&instance).cloned();
        let Some((say, cancelled)) = submitted else {
            return Err(Refusal::new(-32601, "unknown child"));
        };
        let state = |cancelled: bool| InstanceState {
            phase: if cancelled { "stopping" } else { "running" }.into(),
            outcome: String::new(),
            cancellation_requested: cancelled,
            attempt: 1,
        };
        match control.action {
            WireChildAction::Status => Ok(state(cancelled).encode()),
            WireChildAction::Cancel => {
                self.submitted.insert(instance, (say, true));
                Ok(state(true).encode())
            }
            WireChildAction::Join if cancelled => Err(Refusal::new(-32606, "cancelled")),
            WireChildAction::Join => Ok(Echoed { said: say }.encode()),
            WireChildAction::Release | WireChildAction::Detach => Ok(Vec::new()),
        }
    }
}

impl Session {
    async fn phase(&mut self, command: WireCommand) -> WireAnswer {
        let payload = command.encode().unwrap();
        let operation = async |accessor: &wasmtime::component::Accessor<Stub>| {
            self.bindings.call_operate(accessor, "sandbox_guest".to_owned(), payload).await
        };
        let bytes = self
            .store
            .run_concurrent(operation)
            .await
            .expect("the store drives the current operation")
            .expect("the export does not trap")
            .expect("the bridge accepts the command");
        WireAnswer::decode(&bytes).unwrap()
    }
    async fn call<D: harmos_domain::operations::OperationDeclaration>(
        &mut self, invocation: u64, payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let prepared = self
            .phase(WireCommand::Prepare {
                invocation,
                operation: harmos_domain::QualifiedId::qualify("sandbox_guest", D::ID).unwrap().as_str().to_owned(),
                fingerprint: (harmos_domain::operations::Entry::of::<D>().fingerprint)().unwrap().0,
                generation: 1,
                payload,
            })
            .await;
        if let WireAnswer::Failed { failure } = prepared {
            return Err(failure.cause().to_owned());
        }
        let initialized = self.phase(WireCommand::Initialize { invocation, context: [1; 32] }).await;
        assert!(matches!(initialized, WireAnswer::Ready { .. }), "{initialized:?}");
        let answer = self.phase(WireCommand::Attempt { invocation, context: [1; 32], values: Vec::new() }).await;
        let finalized = self.phase(WireCommand::Finalize { invocation }).await;
        assert!(matches!(finalized, WireAnswer::Ready { .. }), "{finalized:?}");
        match answer {
            WireAnswer::Ready { payload } => Ok(payload),
            WireAnswer::Failed { failure } => Err(failure.cause().to_owned()),
            WireAnswer::Stream { .. } => panic!("unexpected stream response to unary phase"),
        }
    }
    async fn initialize(&mut self, config: &str) -> Result<(), String> {
        let request = InitializeRequest {
            config_json: config.as_bytes().to_vec(),
            generation_context: vec![2; 32],
            assignments: harmos_sdk::operations::AssignedOperation::encode(&[
                harmos_sdk::operations::AssignedOperation {
                    operation: "provider.provider_echo".into(),
                    fingerprint: <sandbox::ProviderEchoOperation as harmos_domain::operations::OperationDeclaration>::definition()
                        .fingerprint
                        .0,
                    generation: 1,
                },
                harmos_sdk::operations::AssignedOperation {
                    operation: "host.host_publish".into(),
                    fingerprint: <harmos_operations::streams::Publish as harmos_domain::operations::OperationDeclaration>::definition()
                        .fingerprint
                        .0,
                    generation: 1,
                },
            ])
            .unwrap(),
        };
        self.call::<harmos_operations::lifecycle::Initialize>(1, request.encode()).await.map(|_| ())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn initialize(guest: &[u8], stub: Stub, config: &str) -> (Stub, Result<(), String>) {
    let mut session = instantiate(guest, stub).await;
    let answered = session.initialize(config).await;
    (session.store.into_data(), answered)
}

async fn instantiate(guest: &[u8], stub: Stub) -> Session {
    let engine = engine();
    let component = Component::new(engine, guest).expect("the fixture is a real component");
    let mut linker = Linker::new(engine);
    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |stub| stub).expect("the world's imports are the ones this host offers");
    let mut store = Store::new(engine, stub);
    let bindings = Plugin::instantiate_async(&mut store, &component, &linker).await.expect("the guest instantiates");
    Session { store, bindings }
}

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        Engine::new(&config).expect("the component engine starts")
    })
}

/* === Tests === */

#[tokio::test]
async fn publication_crosses_row() {
    let _report = crate::report::system();
    let (stub, answered) = initialize(&support::sandbox(), Stub::default(), r#""publications""#).await;

    assert_eq!(answered, Ok(()));
    assert_eq!(stub.calls, [Call::Publish { stream: "progress".into(), payload: Echoed { said: "initialize".into() }.encode() }]);
}

#[tokio::test]
async fn registration_precedes_init() {
    let _report = crate::report::system();
    let guest = support::sandbox();
    let declaration = Declaration::inspect(&guest, Facility::Guest).unwrap();
    let required = declaration
        .contracts
        .requires
        .iter()
        .map(|required| {
            let operation = required.contract.operation();
            (operation.id.as_str(), required.grants.len())
        })
        .collect::<Vec<_>>();
    assert_eq!(required, [("provider_echo", 0), ("host_publish", 0)]);

    let mut session = instantiate(&guest, Stub::default()).await;
    let registrations = session
        .bindings
        .call_register(&mut session.store)
        .await
        .expect("registration does not trap")
        .expect("the guest accepts its static catalog");
    assert_eq!(
        registrations.iter().map(|entry| (entry.namespace.as_str(), entry.schema)).collect::<Vec<_>>(),
        [("alpha.catalog", 1), ("zeta.catalog", 2)]
    );
    assert_eq!(registrations[0].payload, br#""first""#);
    assert!(session.store.data().calls.is_empty());
}

#[tokio::test]
async fn refusal_preserves_reason() {
    let _report = crate::report::system();
    let refusal = Err(Refusal::new(-32601, "the provider is not selected"));
    let refusing = Stub { provider: VecDeque::from([refusal]), ..Stub::default() };

    let (stub, answered) = initialize(&support::sandbox(), refusing, r#""dependencies""#).await;

    let cause = answered.unwrap_err();
    assert!(cause.contains("the provider is not selected"), "the refusal reason reaches the guest: {cause}");
    assert_eq!(stub.calls, [Call::Provider { say: "initialize".into() }]);
}

#[tokio::test]
async fn shutdown_current_context() {
    let _report = crate::report::system();
    let initial = Ok(Echoed { said: "initialize".into() });
    let refusal = Err(Refusal::new(-32601, "the lease closed admission"));
    let closing = Stub { provider: VecDeque::from([initial, refusal]), ..Stub::default() };
    let mut session = instantiate(&support::sandbox(), closing).await;
    session.initialize(r#""dependencies""#).await.expect("the admitted dependency call is accepted");
    session.call::<harmos_operations::lifecycle::Shutdown>(2, ().encode()).await.expect("shutdown observes the host refusal");
    assert_eq!(session.store.into_data().calls, [Call::Provider { say: "initialize".into() }, Call::Provider { say: "cleanup".into() },]);
}

#[tokio::test]
async fn submitted_children_controls() {
    let _report = crate::report::system();
    let (stub, answered) = initialize(&support::sandbox(), Stub::default(), r#""submit""#).await;
    assert_eq!(answered, Ok(()));
    let submit = |say: &str, instance| Call::Submit { say: say.into(), instance };
    let control = |instance, action| Call::Control { instance, action };
    assert_eq!(
        stub.calls[..7],
        [
            submit("joined", 1),
            control(1, WireChildAction::Status),
            control(1, WireChildAction::Join),
            submit("cancelled", 2),
            control(2, WireChildAction::Cancel),
            control(2, WireChildAction::Join),
            submit("dropped", 3),
        ]
    );
    for expected in [control(3, WireChildAction::Release), submit("detached", 4), control(4, WireChildAction::Detach)] {
        assert!(stub.calls.contains(&expected), "{expected:?}");
    }
    assert_eq!(stub.calls.len(), 10);
}

#[tokio::test]
async fn unbound_operations_refused() {
    let _report = crate::report::system();
    use harmos_guest::operations::FailureKind;
    let mut session = instantiate(&support::component(), Stub::default()).await;
    session.initialize("null").await.unwrap();
    for (invocation, operation) in [(2, "recorder_start"), (3, "harmos_register")] {
        let answer = session
            .phase(WireCommand::Prepare {
                invocation,
                operation: harmos_domain::QualifiedId::qualify("sandbox_guest", operation).unwrap().as_str().to_owned(),
                fingerprint: [0; 32],
                generation: 1,
                payload: vec![],
            })
            .await;
        assert!(
            matches!(&answer, WireAnswer::Failed { failure } if failure.code() == FailureKind::Permanent && failure.cause().contains(operation)),
            "{answer:?}"
        );
    }
    assert!(session.store.data().calls.is_empty());
}
