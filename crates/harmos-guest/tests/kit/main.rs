/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../../examples/test_component/contract.rs"]
mod contract;
mod plugin;

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
use contract::{Heard, Metrics, Note, Progress};
use harmos::guest::capabilities::Refusal as BridgeRefusal;
use harmos_contract::builtin::InstanceState;
use harmos_contract::inventory::{Declaration, DeclarationError, Facility};
use harmos_contract::operations::host::{self, Fetched, Header, Measured, Receipt, Remembered};
use harmos_contract::operations::{
    OperationDeclaration, WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl,
    WireCommand, lifecycle::InitializeRequest,
};
use harmos_contract::{Message, Refusal};
use support::{component, legacy_component, route_component};

/* === Constants === */

/* === Macros === */
wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
    exports: { default: async },
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Stub {
    calls: Vec<Call>,
    invokes: VecDeque<Result<Receipt, Refusal>>,
    measured: Vec<Measured>,
    remembered: Vec<Remembered>,
    fetched: Option<Fetched>,
    // Submitted children by instance id, and whether each was cancelled.
    submitted: std::collections::BTreeMap<u64, bool>,
}

struct Session {
    store: Store<Stub>,
    bindings: Plugin,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq)]
enum Call {
    Invoke {
        grant: String,
        key: String,
        payload: Vec<u8>,
    },
    Record {
        name: String,
        version: u32,
        payload: Vec<u8>,
    },
    Publish {
        stream: String,
        payload: Vec<u8>,
    },
    Survey {
        stream: String,
        first: u64,
        last: u64,
        aggregate: String,
    },
    Recall {
        name: String,
        after: u64,
        limit: u32,
    },
    Egress {
        name: String,
        method: String,
        path: String,
    },
    Submit {
        key: String,
        instance: u64,
    },
    Control {
        instance: u64,
        action: WireChildAction,
    },
}

/* === Traits === */

/* === Implementations === */
impl Stub {
    fn receipt(position: u64) -> Receipt {
        Receipt {
            position,
            correlation: position,
        }
    }

    fn invoked(&self) -> Vec<&str> {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Call::Invoke { key, .. } => Some(key.as_str()),
                _ => None,
            })
            .collect()
    }
}

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
                message: error.message().to_owned(),
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
            "host.invoke" => {
                let request = host::InvokeRequest::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Invoke {
                    grant: request.grant,
                    key: request.key,
                    payload: request.payload,
                });
                self.invokes
                    .pop_front()
                    .unwrap_or_else(|| Ok(Self::receipt(1)))
                    .map(|receipt| receipt.encode_to_vec())
            }
            "host.record_self" => {
                let request = host::RecordRequest::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Record {
                    name: request.name,
                    version: request.version,
                    payload: request.payload,
                });
                Ok(Self::receipt(7).encode_to_vec())
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
            "host.recall_self" => {
                let request = host::RecallRequest::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Recall {
                    name: request.name,
                    after: request.after,
                    limit: request.limit,
                });
                Ok(host::RecallResponse {
                    rows: self.remembered.clone(),
                }
                .encode_to_vec())
            }
            "host.egress" => {
                let request = host::EgressRequest::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Egress {
                    name: request.name,
                    method: request.method,
                    path: request.path,
                });
                self.fetched
                    .clone()
                    .ok_or_else(|| Refusal::new(-32601, "this stub performs no requests"))
                    .map(|response| response.encode_to_vec())
            }
            "host.survey" => {
                let request = host::SurveyRequest::decode(&call.payload).map_err(Refusal::of)?;
                self.calls.push(Call::Survey {
                    stream: request.stream,
                    first: request.first,
                    last: request.last,
                    aggregate: request.aggregate,
                });
                Ok(host::SurveyResponse {
                    rows: self.measured.clone(),
                }
                .encode_to_vec())
            }
            _ => Err(Refusal::new(-32601, "no operation scope")),
        }
    }

    fn submit(&mut self, call: WireChildCall) -> Result<Vec<u8>, Refusal> {
        assert_eq!(call.operation, "host.invoke");
        let request = host::InvokeRequest::decode(&call.payload).map_err(Refusal::of)?;
        let instance = self.submitted.len() as u64 + 1;
        self.submitted.insert(instance, false);
        self.calls.push(Call::Submit {
            key: request.key,
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
        let cancelled = *self
            .submitted
            .get(&instance)
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
                self.submitted.insert(instance, true);
                Ok(state(true).encode_to_vec())
            }
            WireChildAction::Join if cancelled => Err(Refusal::new(-32606, "cancelled")),
            WireChildAction::Join => Ok(Self::receipt(instance).encode_to_vec()),
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
        if let WireAnswer::Failed { message, .. } = prepared {
            return Err(message);
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
            WireAnswer::Failed { message, .. } => Err(message),
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
async fn initialize(stub: Stub, config: &str) -> (Stub, Result<(), String>) {
    let mut session = instantiate(stub).await;
    let answered = session.initialize(config).await;
    (session.store.into_data(), answered)
}

async fn instantiate(stub: Stub) -> Session {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    let engine = ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        Engine::new(&config).expect("the component engine starts")
    });
    static COMPONENT: OnceLock<Component> = OnceLock::new();
    let component = COMPONENT.get_or_init(|| {
        Component::new(engine, component()).expect("the fixture is a real component")
    });
    let mut linker = Linker::new(engine);
    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |stub| stub)
        .expect("the world's imports are the ones this host offers");
    let mut store = Store::new(engine, stub);
    let bindings = Plugin::instantiate_async(&mut store, component, &linker)
        .await
        .expect("the guest instantiates");
    Session { store, bindings }
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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("host_calls_cross_arguments", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (stub, answered) = initialize(Stub::default(), r#""invoke""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [Call::Invoke {
                            grant: "counter.increment".into(),
                            key: "accepted".into(),
                            payload: br#"{"amount":1}"#.to_vec(),
                        }]
                    );

                    let (stub, answered) = initialize(Stub::default(), r#""record""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [Call::Record {
                            name: "finding".into(),
                            version: 1,
                            payload: br#"{"message":"landed"}"#.to_vec(),
                        }]
                    );

                    let (stub, answered) = initialize(Stub::default(), r#""publish""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [
                            Call::Publish {
                                stream: "progress".into(),
                                payload: Progress { percent: 25 }.encode_to_vec(),
                            },
                            Call::Publish {
                                stream: "metrics".into(),
                                payload: Metrics { count: 3 }.encode_to_vec(),
                            },
                        ]
                    );
                });
        });
    }
    {
        scenario_runtime::check("registration_precedes_init", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let declaration = Declaration::inspect(&component(), Facility::Guest).unwrap();
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
                    assert_eq!(
                        required,
                        [
                            ("host.publish", 0),
                            ("host.record_self", 0),
                            ("host.recall_self", 0),
                            ("host.invoke", 0),
                            ("host.egress", 1),
                            ("host.survey", 1),
                        ]
                    );
                    let mut session = instantiate(Stub::default()).await;
                    let registrations = session
                        .bindings
                        .call_register(&mut session.store)
                        .await
                        .expect("registration does not trap")
                        .expect("the guest accepts its static catalog");

                    assert_eq!(registrations.len(), 1);
                    assert_eq!(registrations[0].namespace, "example.catalog");
                    assert_eq!(registrations[0].schema, 1);
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&registrations[0].payload)
                            .unwrap(),
                        serde_json::json!({ "suite": "guest" })
                    );
                });
        });
    }
    {
        scenario_runtime::check("receipts_and_measurements", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    // The guest keys its edit on the position its own fact landed at, so a
                    // receipt that did not cross back intact would show in the key.
                    let (stub, answered) = initialize(Stub::default(), r#""invoke-fresh""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls[1],
                        Call::Invoke {
                            grant: "counter.increment".into(),
                            key: "fresh-7".into(),
                            payload: br#"{"amount":1}"#.to_vec(),
                        }
                    );

                    // The guest checks every field of the page that came back and refuses the
                    // activation when one of them did not survive, so a clean answer *is* the
                    // crossing.
                    let remembering = Stub {
                        remembered: vec![Remembered {
                            position: 9,
                            name: "finding".into(),
                            version: 2,
                            payload: br#"{"message":"kept"}"#.to_vec(),
                        }],
                        ..Stub::default()
                    };
                    let (stub, answered) = initialize(remembering, r#""recall-wire""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [Call::Recall {
                            name: "finding".into(),
                            after: 3,
                            limit: 5,
                        }]
                    );

                    // The guest checks the status and writes the body back as a fact, so a
                    // clean answer is the whole crossing: name, method, path out; status and
                    // body back.
                    let reaching = Stub {
                        fetched: Some(Fetched {
                            status: 200,
                            headers: vec![Header {
                                name: "content-type".into(),
                                value: "text/plain".into(),
                            }],
                            body: b"vendor answered".to_vec(),
                        }),
                        ..Stub::default()
                    };
                    let (stub, answered) = initialize(reaching, r#""egress""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [
                            Call::Egress {
                                name: "vendor".into(),
                                method: "get".into(),
                                path: "/reading".into(),
                            },
                            Call::Record {
                                name: "finding".into(),
                                version: 1,
                                payload: br#"{"message":"vendor answered"}"#.to_vec(),
                            },
                        ]
                    );

                    let surveying = Stub {
                        measured: vec![Measured {
                            field: "value".into(),
                            value: 4.5,
                        }],
                        ..Stub::default()
                    };
                    let (stub, answered) = initialize(surveying, r#""survey""#).await;
                    assert_eq!(answered, Ok(()));
                    assert_eq!(
                        stub.calls,
                        [
                            Call::Survey {
                                stream: "capture".into(),
                                first: 1,
                                last: 8,
                                aggregate: "mean".into(),
                            },
                            // The guest truncated the statistic the host measured, which it
                            // could only do if the float crossed as a float.
                            Call::Publish {
                                stream: "metrics".into(),
                                payload: Metrics { count: 4 }.encode_to_vec(),
                            },
                        ]
                    );
                });
        });
    }
    {
        scenario_runtime::check("refusal_preserves_reason", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let refusing = Stub {
        invokes: VecDeque::from([Err(Refusal::new(
            -32601,
            "grant \"counter.increment\" was not declared",
        ))]),
        ..Stub::default()
    };

    let (_, answered) = initialize(refusing, r#""invoke""#).await;

    assert_eq!(
        answered,
        Err("carrier handler failed: plugin lifecycle machinery failed: grant \"counter.increment\" was not declared (-32601)".to_owned()),
        "both halves of the refusal record reach the guest"
    );

});
        });
    }
    {
        scenario_runtime::check("submitted_children_controls", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (stub, answered) = initialize(Stub::default(), r#""submit""#).await;
                    assert_eq!(answered, Ok(()));
                    let submit = |key: &str, instance| Call::Submit {
                        key: key.into(),
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
                });
        });
    }
    {
        scenario_runtime::check("initialization_refusal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (stub, answered) =
                        initialize(Stub::default(), r#""nothing-the-guest-knows""#).await;

                    assert_eq!(
        answered,
        Err(
            "carrier handler failed: plugin lifecycle machinery failed: unknown test mode"
                .to_owned()
        )
    );
                    assert!(
                        stub.calls.is_empty(),
                        "a refused initialization asked the host for nothing"
                    );
                });
        });
    }
    {
        scenario_runtime::check("shutdown_current_context", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let closing = Stub {
                        invokes: VecDeque::from([
                            Ok(Stub::receipt(1)),
                            Err(Refusal::new(-32601, "the lease closed admission")),
                        ]),
                        ..Stub::default()
                    };
                    let mut session = instantiate(closing).await;
                    session
                        .initialize(r#""revoke""#)
                        .await
                        .expect("the admitted edit is accepted");
                    session
                        .call(2, "harmos.shutdown", ().encode_to_vec())
                        .await
                        .expect("shutdown observes the host refusal");
                    assert_eq!(
                        session.store.into_data().invoked(),
                        ["admitted", "after-revoke"]
                    );
                });
        });
    }
    {
        scenario_runtime::check("unbound_operations_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use harmos_guest::operations::Failure;
    let mut session = instantiate(Stub::default()).await;
    session.initialize(r#""idle""#).await.unwrap();
    for (invocation, operation) in [(2, "recorder.start"), (3, "harmos.register")] {
        let answer = session
            .phase(WireCommand::Prepare {
                invocation,
                operation: operation.into(),
                payload: vec![],
            })
            .await;
        assert!(
            matches!(&answer, WireAnswer::Failed { failure: Failure::Permanent, message } if message.contains(operation)),
            "{answer:?}"
        );
    }

});
        });
    }
    {
        scenario_runtime::check("listener_bridge_resources", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos_contract::journal::Position;
                    use harmos_contract::listeners::{Context, Delivery};
                    for (mode, refused) in [("idle", false), ("refuse-delivery", true)] {
                        let mut session = instantiate(Stub::default()).await;
                        session
                            .initialize(&serde_json::to_string(mode).unwrap())
                            .await
                            .unwrap();
                        let request = Delivery::new(
                            Context::Record {
                                position: Position(19),
                            },
                            Note {
                                note: "heard".into(),
                            },
                        )
                        .unwrap();
                        let result = session.call(2, Heard::ID, request.encode_to_vec()).await;
                        if refused {
                            assert_eq!(
                result,
                Err("carrier handler failed: this delivery is one the guest refuses".into())
            );
                            assert!(session.store.data().calls.is_empty());
                        } else {
                            assert!(result.is_ok());
                            assert_eq!(
                session.store.data().calls,
                [Call::Record {
                    name: "finding".into(),
                    version: 1,
                    payload: serde_json::to_vec(
                        &serde_json::json!({"message": format!("{} heard", contract::NOTE)})
                    )
                    .unwrap()
                }]
            );
                        }
                    }
                });
        });
    }
    {
        scenario_runtime::check("world_hash_unchanged", || {
            // Selected Operation Catalogs Do Not Change The Component World
            let routeless = world(&component());
            let routed = world(&route_component());

            assert_eq!(
                digest(&routeless),
                digest(&routed),
                "declaring operations moved the world:\n{routeless}\n---\n{routed}"
            );
            assert!(
                routed.contains("operate"),
                "the world both guests carry is the one with `operate` on it:\n{routed}"
            );
        });
    }
    {
        scenario_runtime::check("retired_metadata_refused", || {
            assert!(matches!(
                Declaration::inspect(&legacy_component(), Facility::Guest),
                Err(DeclarationError::IncompatiblePayload)
            ));
        });
    }
    {
        scenario_runtime::check("inventory_owner_isolation", || {
            let bytes = route_component();
            let declared = Declaration::inspect(&bytes, Facility::Guest)
                .expect("the real Wasm inventory reads without instantiation");
            assert_eq!(declared.id, "route-guest");
            assert_eq!(
                declared
                    .operations
                    .iter()
                    .map(|operation| (operation.id.as_str(), operation.policy.timeout.as_millis()))
                    .collect::<Vec<_>>(),
                [
                    ("probe.echo", 2000),
                    ("probe.decline", 2000),
                    ("probe.spin", 600000),
                    ("probe.wait", 5),
                    ("probe.fail", 2000),
                ]
            );
        });
    }
}

/* === Tests === */
