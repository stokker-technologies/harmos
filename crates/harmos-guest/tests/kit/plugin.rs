/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};
// Local Imports
use super::{Stub, support};
use harmos_guest::Message;
use harmos_guest::operations::{WireAnswer, WireCommand, lifecycle::InitializeRequest};

/* === Constants === */

/* === Macros === */
wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
    exports: { default: async },
    with: {
        "harmos:guest/operation-context": crate::harmos::guest::operation_context,
        "harmos:guest/capabilities": crate::harmos::guest::capabilities,
    },
});

/* === Type Aliases === */

/* === Structs === */
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
struct Reply {
    value: String,
}

struct Session {
    store: Store<Stub>,
    bindings: Plugin,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Session {
    async fn phase(&mut self, command: WireCommand) -> WireAnswer {
        let payload = command.encode().unwrap();
        let bytes = self
            .store
            .run_concurrent(async |accessor| self.bindings.call_operate(accessor, payload).await)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        WireAnswer::decode(&bytes).unwrap()
    }
    async fn call(&mut self, invocation: u64, operation: &str, payload: Vec<u8>) -> Vec<u8> {
        assert!(matches!(
            self.phase(WireCommand::Prepare {
                invocation,
                operation: operation.to_owned(),
                payload,
            })
            .await,
            WireAnswer::Ready { .. }
        ));
        for _ in 0..2 {
            assert!(matches!(
                self.phase(WireCommand::Initialize { invocation, context: [1; 32] }).await,
                WireAnswer::Ready { payload } if payload.is_empty()
            ));
        }
        let answer = self
            .phase(WireCommand::Attempt {
                invocation,
                context: [0; 32],
                values: Vec::new(),
            })
            .await;
        let WireAnswer::Ready { payload } = answer else {
            panic!("{answer:?}")
        };
        assert!(matches!(
            self.phase(WireCommand::Finalize { invocation }).await,
            WireAnswer::Ready { .. }
        ));
        payload
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("sdk_lifecycle_retained", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let bytes = support::plugin_component();
                    let declaration = harmos_contract::inventory::Declaration::inspect(
                        &bytes,
                        harmos_contract::inventory::Facility::Guest,
                    )
                    .unwrap();
                    let contracts = &declaration.contracts;
                    assert_eq!(declaration.id, "unified-guest");
                    assert_eq!(contracts.provides.len(), 1);
                    assert!(contracts.requires.is_empty());
                    assert_eq!(declaration.operations[0].id.as_str(), "fixture.read");

                    let mut config = Config::new();
                    config.wasm_component_model(true);
                    config.wasm_component_model_async(true);
                    let engine = Engine::new(&config).unwrap();
                    let component = Component::new(&engine, bytes).unwrap();
                    let mut linker = Linker::new(&engine);
                    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |stub| stub).unwrap();
                    let exports = component.component_type();
                    for name in ["initialize", "shutdown"] {
                        assert!(
                            exports.get_export(&engine, name).is_none(),
                            "the unified component exports no independent {name} function"
                        );
                    }
                    let mut store = Store::new(&engine, Stub::default());
                    let bindings = Plugin::instantiate_async(&mut store, &component, &linker)
                        .await
                        .unwrap();
                    let mut session = Session { store, bindings };

                    assert!(
                        matches!(
                            session
                                .phase(WireCommand::Prepare {
                                    invocation: 1,
                                    operation: "fixture.read".to_owned(),
                                    payload: vec![],
                                })
                                .await,
                            WireAnswer::Failed { .. }
                        ),
                        "ordinary handlers are absent before SDK initialization"
                    );
                    assert!(
                        matches!(
                            session
                                .phase(WireCommand::Prepare {
                                    invocation: 7,
                                    operation: "harmos.initialize".to_owned(),
                                    payload: InitializeRequest {
                                        config_json: b"null".to_vec(),
                                        generation_context: Vec::new()
                                    }
                                    .encode_to_vec(),
                                })
                                .await,
                            WireAnswer::Ready { .. }
                        ),
                        "inert preparation only retains the original request"
                    );
                    assert!(matches!(
                        session
                            .phase(WireCommand::Initialize {
                                invocation: 7,
                                context: [0; 32]
                            })
                            .await,
                        WireAnswer::Ready { .. }
                    ));
                    assert!(
                        matches!(
                            session
                                .phase(WireCommand::Attempt {
                                    invocation: 7,
                                    context: [0; 32],
                                    values: Vec::new(),
                                })
                                .await,
                            WireAnswer::Failed { .. }
                        ),
                        "the first run validates configuration before plugin initialization"
                    );
                    assert!(
                        matches!(
                            session.phase(WireCommand::Finalize { invocation: 7 }).await,
                            WireAnswer::Ready { .. }
                        ),
                        "original finalization releases the failed invocation's capacity"
                    );
                    let registrations = session
                        .bindings
                        .call_register(&mut session.store)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(registrations.len(), 2);
                    assert_eq!(registrations[0].namespace, "alpha.catalog");
                    assert_eq!(registrations[0].schema, 1);
                    assert_eq!(registrations[0].payload, br#""first""#);
                    assert_eq!(registrations[1].namespace, "zeta.catalog");
                    assert!(
                        matches!(
                            session
                                .phase(WireCommand::Prepare {
                                    invocation: 8,
                                    operation: "fixture.read".to_owned(),
                                    payload: vec![],
                                })
                                .await,
                            WireAnswer::Failed { .. }
                        ),
                        "metadata collection does not initialize resources or ordinary handlers"
                    );
                    let request = InitializeRequest {
                        config_json: br#""same-owner""#.to_vec(),
                        generation_context: Vec::new(),
                    };
                    assert!(
                        session
                            .call(2, "harmos.initialize", request.encode_to_vec())
                            .await
                            .is_empty()
                    );
                    let value = session.call(3, "fixture.read", vec![]).await;
                    assert_eq!(
                        Reply::decode(&value).unwrap().value,
                        "same-owner:initialized:operation-initialized:1"
                    );
                    assert!(session.call(4, "harmos.shutdown", vec![]).await.is_empty());
                    assert!(
                        matches!(
                            session
                                .phase(WireCommand::Prepare {
                                    invocation: 5,
                                    operation: "fixture.read".to_owned(),
                                    payload: vec![],
                                })
                                .await,
                            WireAnswer::Failed { .. }
                        ),
                        "shutdown revokes ordinary admission"
                    );
                    assert!(
                        session.call(6, "harmos.shutdown", vec![]).await.is_empty(),
                        "SDK shutdown is idempotent on the same closed owner"
                    );
                });
        });
    }
}

/* === Tests === */
