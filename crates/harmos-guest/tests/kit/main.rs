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
use harmos_contract::builtin::InstanceState;
use harmos_contract::inventory::{Declaration, Facility};
use harmos_contract::operations::host;
use harmos_contract::operations::{
    WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCommand,
    lifecycle::InitializeRequest,
};
use harmos_contract::{Message, Refusal};
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
    Provider {
        say: String,
    },
    Publish {
        stream: String,
        payload: Vec<u8>,
    },
    Other {
        operation: String,
    },
    Submit {
        say: String,
        instance: u64,
    },
    Control {
        instance: u64,
        action: WireChildAction,
    },
}

/* === Traits === */

/* === Implementations === */
impl harmos::guest::capabilities::Host for Stub {}

impl harmos::guest::operation_context::Host for Stub {}

impl<T: Send> harmos::guest::operation_context::HostWithStore<T> for HasSelf<Stub> {
    async fn child(
        accessor: &wasmtime::component::Accessor<T, Self>,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, BridgeRefusal> {
        accessor
            .with(|mut access| access.get().child(payload))
            .map_err(|error| BridgeRefusal {
                code: error.code().to_string(),
                failure: error.failure().encode(),
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
        assert_ne!(
            call.context, [0; 32],
            "the SDK forwards the current finite calling context"
        );
        match call.operation.as_str() {
            "provider.echo" => {
                let request = Echo::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Provider {
                    say: request.say.clone(),
                });
                self.provider
                    .pop_front()
                    .unwrap_or(Ok(Echoed { said: request.say }))
                    .map(|answer| answer.encode_to_vec())
            }
            "host.publish" => {
                let request = host::Publication::decode(&call.payload).map_err(Refusal::of)?;
                assert_eq!(request.schema, 1);
                assert_eq!(
                    request.sequence, None,
                    "the guest asks its original lease to derive the sequence"
                );
                assert_eq!(request.published_unix_micros, None);
                assert_eq!(request.context, None);
                assert!(request.following.is_empty());
                self.calls.push(Call::Publish {
                    stream: request.stream,
                    payload: request.payload,
                });
                Ok(().encode_to_vec())
            }
            operation => {
                self.calls.push(Call::Other {
                    operation: operation.to_owned(),
                });
                Err(Refusal::new(-32601, "no operation scope"))
            }
        }
    }
}

impl Stub {
    fn submit(&mut self, call: WireChildCall) -> Result<Vec<u8>, Refusal> {
        assert_eq!(call.operation, "provider.echo");
        let request = Echo::decode(&call.payload).map_err(Refusal::of)?;
        let instance = self.submitted.len() as u64 + 1;
        self.submitted
            .insert(instance, (request.say.clone(), false));
        self.calls.push(Call::Submit {
            say: request.say,
            instance,
        });
        Ok(instance.encode_to_vec())
    }

    fn control(&mut self, control: WireChildControl) -> Result<Vec<u8>, Refusal> {
        assert_ne!(control.context, [0; 32], "controls name the attempt");
        let instance = control.instance;
        self.calls.push(Call::Control {
            instance,
            action: control.action,
        });
        let (say, cancelled) = self
            .submitted
            .get(&instance)
            .cloned()
            .ok_or_else(|| Refusal::new(-32601, "unknown child"))?;
        let state = |cancelled: bool| InstanceState {
            phase: if cancelled { "stopping" } else { "running" }.into(),
            outcome: String::new(),
            cancellation_requested: cancelled,
            attempt: 1,
        };
        match control.action {
            WireChildAction::Status => Ok(state(cancelled).encode_to_vec()),
            WireChildAction::Cancel => {
                self.submitted.insert(instance, (say, true));
                Ok(state(true).encode_to_vec())
            }
            WireChildAction::Join if cancelled => Err(Refusal::new(-32606, "cancelled")),
            WireChildAction::Join => Ok(Echoed { said: say }.encode_to_vec()),
            WireChildAction::Release | WireChildAction::Detach => Ok(Vec::new()),
        }
    }
}

impl Session {
    async fn phase(&mut self, command: WireCommand) -> WireAnswer {
        let payload = command.encode().unwrap();
        let bytes = self
            .store
            .run_concurrent(async |accessor| self.bindings.call_operate(accessor, payload).await)
            .await
            .expect("the store drives the current operation")
            .expect("the export does not trap")
            .expect("the bridge accepts the command");
        WireAnswer::decode(&bytes).unwrap()
    }
    async fn call(
        &mut self,
        invocation: u64,
        operation: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let prepared = self
            .phase(WireCommand::Prepare {
                invocation,
                operation: operation.into(),
                payload,
            })
            .await;
        if let WireAnswer::Failed { failure } = prepared {
            return Err(failure.cause().to_owned());
        }
        let initialized = self
            .phase(WireCommand::Initialize {
                invocation,
                context: [1; 32],
            })
            .await;
        assert!(
            matches!(initialized, WireAnswer::Ready { .. }),
            "{initialized:?}"
        );
        let answer = self
            .phase(WireCommand::Attempt {
                invocation,
                context: [1; 32],
                values: Vec::new(),
            })
            .await;
        let finalized = self.phase(WireCommand::Finalize { invocation }).await;
        assert!(
            matches!(finalized, WireAnswer::Ready { .. }),
            "{finalized:?}"
        );
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
        };
        self.call(1, "harmos.initialize", request.encode_to_vec())
            .await
            .map(|_| ())
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
    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |stub| stub)
        .expect("the world's imports are the ones this host offers");
    let mut store = Store::new(engine, stub);
    let bindings = Plugin::instantiate_async(&mut store, &component, &linker)
        .await
        .expect("the guest instantiates");
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

fn world(component: &[u8]) -> String {
    let decoded = wit_component::decode(component).expect("the fixture is a real component");
    let wit_component::DecodedWasm::Component(resolve, world) = decoded else {
        panic!("a component decodes as a component");
    };
    let package = resolve.worlds[world]
        .package
        .expect("a decoded world belongs to a package");
    let mut printer = wit_component::WitPrinter::default();
    printer
        .print(&resolve, package, &[])
        .expect("the decoded world prints");

    printer.output.to_string()
}

fn digest(world: &str) -> u64 {
    world.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[tokio::test]
async fn host_calls_cross_arguments() {
    let guest = support::sandbox();

    // Publication Crosses As The Declared Row
    let (stub, answered) = initialize(&guest, Stub::default(), r#""publications""#).await;
    assert_eq!(answered, Ok(()));
    assert_eq!(
        stub.calls,
        [Call::Publish {
            stream: "progress".into(),
            payload: Echoed {
                said: "initialize".into()
            }
            .encode_to_vec(),
        }]
    );

    // A Dependency Call Crosses Out And Its Answer Crosses Back
    let (stub, answered) = initialize(&guest, Stub::default(), r#""dependencies""#).await;
    assert_eq!(answered, Ok(()));
    assert_eq!(
        stub.calls,
        [Call::Provider {
            say: "initialize".into()
        }]
    );
}

#[tokio::test]
async fn registration_precedes_init() {
    let guest = support::sandbox();
    let declaration = Declaration::inspect(&guest, Facility::Guest).unwrap();
    let required = declaration
        .contracts
        .requires
        .iter()
        .map(|required| {
            (
                required.contract.operation().id.as_str(),
                required.grants.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(required, [("provider.echo", 0), ("host.publish", 0)]);

    let mut session = instantiate(&guest, Stub::default()).await;
    let registrations = session
        .bindings
        .call_register(&mut session.store)
        .await
        .expect("registration does not trap")
        .expect("the guest accepts its static catalog");
    assert_eq!(
        registrations
            .iter()
            .map(|entry| (entry.namespace.as_str(), entry.schema))
            .collect::<Vec<_>>(),
        [("alpha.catalog", 1), ("zeta.catalog", 2)]
    );
    assert_eq!(registrations[0].payload, br#""first""#);
    assert!(session.store.data().calls.is_empty());
}

#[tokio::test]
async fn refusal_preserves_reason() {
    let refusing = Stub {
        provider: VecDeque::from([Err(Refusal::new(-32601, "the provider is not selected"))]),
        ..Stub::default()
    };

    let (stub, answered) = initialize(&support::sandbox(), refusing, r#""dependencies""#).await;

    let cause = answered.unwrap_err();
    assert!(
        cause.contains("the provider is not selected"),
        "the refusal reason reaches the guest: {cause}"
    );
    assert_eq!(
        stub.calls,
        [Call::Provider {
            say: "initialize".into()
        }]
    );
}

#[tokio::test]
async fn shutdown_current_context() {
    let closing = Stub {
        provider: VecDeque::from([
            Ok(Echoed {
                said: "initialize".into(),
            }),
            Err(Refusal::new(-32601, "the lease closed admission")),
        ]),
        ..Stub::default()
    };
    let mut session = instantiate(&support::sandbox(), closing).await;
    session
        .initialize(r#""dependencies""#)
        .await
        .expect("the admitted dependency call is accepted");
    session
        .call(2, "harmos.shutdown", ().encode_to_vec())
        .await
        .expect("shutdown observes the host refusal");
    assert_eq!(
        session.store.into_data().calls,
        [
            Call::Provider {
                say: "initialize".into()
            },
            Call::Provider {
                say: "cleanup".into()
            },
        ]
    );
}

#[tokio::test]
async fn submitted_children_controls() {
    let (stub, answered) = initialize(&support::sandbox(), Stub::default(), r#""submit""#).await;
    assert_eq!(answered, Ok(()));
    let submit = |say: &str, instance| Call::Submit {
        say: say.into(),
        instance,
    };
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
    for expected in [
        control(3, WireChildAction::Release),
        submit("detached", 4),
        control(4, WireChildAction::Detach),
    ] {
        assert!(stub.calls.contains(&expected), "{expected:?}");
    }
    assert_eq!(stub.calls.len(), 10);
}

#[tokio::test]
async fn initialization_refusal() {
    // The Schematic Guest Takes No Configuration, So Any JSON Value But Null Is Refused
    let (stub, answered) = initialize(
        &support::component(),
        Stub::default(),
        r#""nothing-the-guest-knows""#,
    )
    .await;

    assert!(answered.is_err(), "{answered:?}");
    assert!(
        stub.calls.is_empty(),
        "a refused initialization asked the host for nothing"
    );
}

#[tokio::test]
async fn unbound_operations_refused() {
    use harmos_guest::operations::FailureKind;
    let mut session = instantiate(&support::component(), Stub::default()).await;
    session.initialize("null").await.unwrap();
    for (invocation, operation) in [(2, "recorder.start"), (3, "harmos.register")] {
        let answer = session
            .phase(WireCommand::Prepare {
                invocation,
                operation: operation.into(),
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

#[test]
fn world_hash_unchanged() {
    // Different Operation Catalogs Do Not Change The Component World
    let schematic = world(&support::component());
    let sandbox = world(&support::sandbox());

    assert_eq!(
        digest(&schematic),
        digest(&sandbox),
        "declaring other operations moved the world:\n{schematic}\n---\n{sandbox}"
    );
    assert!(
        sandbox.contains("operate"),
        "the world both guests carry is the one with `operate` on it:\n{sandbox}"
    );
}

#[test]
fn inventory_owner_isolation() {
    let bytes = support::component();
    let declared = Declaration::inspect(&bytes, Facility::Guest)
        .expect("the real Wasm inventory reads without instantiation");
    assert_eq!(declared.id, "schematic-tools");
    assert!(declared.contracts.requires.is_empty());
    assert_eq!(
        declared
            .operations
            .iter()
            .map(|operation| (operation.id.as_str(), operation.policy.timeout.as_millis()))
            .collect::<Vec<_>>(),
        [
            ("schematic.validate", 10000),
            ("schematic.route", 10000),
            ("schematic.analyze", 10000),
        ]
    );
}

/* === Tests === */
