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

// Third Party Imports
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};
// Local Imports
use super::{Stub, support};
use harmos_guest::Message;
use harmos_guest::operations::{WireAnswer, WireCommand, lifecycle::Initialize};

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
// Mirrors The Schematic Contract's Route Messages; Field Names Fix The Wire Tags
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, Debug, PartialEq)]
struct Request {
    from: Point,
    to: Point,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, Debug, PartialEq)]
struct Response {
    points: Vec<Point>,
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

#[tokio::test]
async fn sdk_lifecycle_retained() {
    let bytes = support::component();
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
    let route = || {
        Request {
            from: Point { x: 0, y: 0 },
            to: Point { x: 3, y: 4 },
        }
        .encode_to_vec()
    };

    assert!(
        matches!(
            session
                .phase(WireCommand::Prepare {
                    invocation: 1,
                    operation: "schematic.route".to_owned(),
                    payload: route(),
                })
                .await,
            WireAnswer::Failed { .. }
        ),
        "ordinary handlers are absent before SDK initialization"
    );
    // The Guest Takes No Configuration, So A JSON String Fails Its First Run
    assert!(
        matches!(
            session
                .phase(WireCommand::Prepare {
                    invocation: 7,
                    operation: "harmos.initialize".to_owned(),
                    payload: Initialize {
                        config_json: br#""unexpected""#.to_vec(),
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
        session.store.data().calls.is_empty(),
        "a refused initialization asked the host for nothing"
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
    assert!(
        registrations.is_empty(),
        "the schematic guest registers no catalog"
    );
    assert!(
        matches!(
            session
                .phase(WireCommand::Prepare {
                    invocation: 8,
                    operation: "schematic.route".to_owned(),
                    payload: route(),
                })
                .await,
            WireAnswer::Failed { .. }
        ),
        "metadata collection does not initialize resources or ordinary handlers"
    );
    let request = Initialize {
        config_json: b"null".to_vec(),
        generation_context: Vec::new(),
    };
    assert!(
        session
            .call(2, "harmos.initialize", request.encode_to_vec())
            .await
            .is_empty()
    );
    let routed = session.call(3, "schematic.route", route()).await;
    assert_eq!(
        Response::decode(&routed).unwrap().points,
        [
            Point { x: 0, y: 0 },
            Point { x: 3, y: 0 },
            Point { x: 3, y: 4 },
        ]
    );
    assert!(session.call(4, "harmos.shutdown", vec![]).await.is_empty());
    assert!(
        matches!(
            session
                .phase(WireCommand::Prepare {
                    invocation: 5,
                    operation: "schematic.route".to_owned(),
                    payload: route(),
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
}

/* === Tests === */
