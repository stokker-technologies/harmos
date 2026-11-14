/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive the Component Lifecycle

/* === Modules === */

mod bindings {
    wasmtime::component::bindgen!({ path: "wit", world: "plugin", exports: { default: async | store } });
}

/* === Re-Exports === */

/* === Imports === */

use super::contract::{EchoOperation, Echoed};
use harmos_protocol::{
    v1 as host,
    v1::{self, guest_child, guest_command, response},
};
use prost::Message as _;
use std::collections::BTreeMap;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Imports {
    limits: Option<wasmtime::StoreLimits>,
    calls: Vec<(String, bool)>,
    generations: Vec<usize>,
    controls: Vec<v1::GuestChildAction>,
    children: BTreeMap<u64, (Vec<u8>, bool)>,
}
struct Session {
    store: Store<Imports>,
    bindings: bindings::Plugin,
    metadata: v1::GuestRegistration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Session {
    async fn new(component: &[u8]) -> Self {
        let mut config = Config::new();
        config.wasm_component_model(true).wasm_component_model_async(true).concurrency_support(true).consume_fuel(true);
        let engine = Engine::new(&config).unwrap();
        let component = Component::new(&engine, component).unwrap();
        let mut linker = Linker::<Imports>::new(&engine);
        linker
            .instance("harmos:guest/operation-context@1.0.0")
            .unwrap()
            .func_wrap_concurrent("child", |accessor, (bytes,): (Vec<u8>,)| {
                Box::pin(async move {
                    let result = accessor.with(|mut access| imported(access.get(), bytes));
                    Ok((result,))
                })
            })
            .unwrap();
        let limits = wasmtime::StoreLimitsBuilder::new()
            .memory_size(64 * 1024 * 1024)
            .memories(1)
            .table_elements(65_536)
            .tables(8)
            .instances(8)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &engine,
            Imports {
                limits: Some(limits),
                ..Default::default()
            },
        );
        store.limiter(|imports| imports.limits.as_mut().unwrap());
        store.set_fuel(10_000_000).unwrap();
        let bindings = bindings::Plugin::instantiate_async(&mut store, &component, &linker).await.unwrap();
        let metadata = store.run_concurrent(async |accessor| bindings.call_register(accessor).await).await.unwrap().unwrap().unwrap();
        let metadata = v1::GuestRegistration::decode(metadata.as_slice()).unwrap();
        Self { store, bindings, metadata }
    }
    async fn phase(&mut self, phase: guest_command::Phase) -> response::Result {
        self.store.set_fuel(10_000_000).unwrap();
        let command = v1::GuestCommand { phase: Some(phase) }.encode_to_vec();
        let bytes = self
            .store
            .run_concurrent(async |accessor| self.bindings.call_operate(accessor, self.metadata.id.clone(), command).await)
            .await
            .unwrap()
            .unwrap();
        v1::Response::decode(bytes.as_slice()).unwrap().result.unwrap()
    }
    async fn prepare(&mut self, id: &str, invocation: u64, payload: Vec<u8>) -> response::Result {
        let catalog = self.metadata.catalog.as_ref().unwrap();
        let fingerprint = if id.starts_with("harmos.") {
            harmos_protocol::guest::lifecycle_fingerprint(id).unwrap()
        } else {
            catalog.operations.iter().find(|operation| operation.id == id).unwrap().fingerprint.clone()
        };
        self.phase(guest_command::Phase::Prepare(v1::GuestPrepare {
            invocation,
            operation: format!("{}.{id}", self.metadata.id),
            fingerprint,
            generation: 1,
            payload,
        }))
        .await
    }
    async fn call(&mut self, id: &str, invocation: u64, payload: Vec<u8>) -> response::Result {
        let prepared = self.prepare(id, invocation, payload).await;
        if matches!(prepared, response::Result::Failure(_)) {
            return prepared;
        }
        self.phase(guest_command::Phase::Initialize(v1::GuestInvocation {
            invocation,
            context: vec![1; 32],
            values: Vec::new(),
        }))
        .await;
        let answer = self
            .phase(guest_command::Phase::Attempt(v1::GuestInvocation {
                invocation,
                context: vec![1; 32],
                values: Vec::new(),
            }))
            .await;
        self.phase(guest_command::Phase::Finalize(invocation)).await;
        answer
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn imported(imports: &mut Imports, bytes: Vec<u8>) -> Result<Vec<u8>, Vec<u8>> {
    let request = harmos_protocol::guest::decode_child(&bytes).unwrap();
    match request.action.unwrap() {
        guest_child::Action::Call(call) => {
            let request = call.request.unwrap();
            imports.calls.push((request.operation.clone(), call.submit));
            imports.generations.push(call.generation_context.len());
            if call.submit {
                let instance = imports.calls.len() as u64;
                // ProviderEchoOperation and Echoed Have One String Field With the Same Tag.
                imports.children.insert(instance, (request.payload, false));
                return Ok(v1::GuestSubmitted { instance }.encode_to_vec());
            }
            if request.operation == "host.example.plugin.host.v1.Progress/Publish" {
                return Ok(Vec::new());
            }
            Ok(request.payload)
        }
        guest_child::Action::Control(control) => {
            let action = control.action();
            imports.controls.push(action);
            let (payload, cancelled) = imports.children.get_mut(&control.instance).unwrap();
            match action {
                v1::GuestChildAction::Status | v1::GuestChildAction::Cancel => {
                    if action == v1::GuestChildAction::Cancel {
                        *cancelled = true;
                    }
                    Ok(host::InstanceState {
                        phase: "running".into(),
                        cancellation_requested: *cancelled,
                        attempt: 1,
                        ..Default::default()
                    }
                    .encode_to_vec())
                }
                v1::GuestChildAction::Join if *cancelled => Err(v1::Failure {
                    classification: v1::Classification::Cancelled as i32,
                    message: "cancelled".into(),
                    ..Default::default()
                }
                .encode_to_vec()),
                v1::GuestChildAction::Join => Ok(payload.clone()),
                v1::GuestChildAction::Release | v1::GuestChildAction::Detach => Ok(Vec::new()),
                _ => unreachable!(),
            }
        }
    }
}
fn world(component: &[u8]) -> String {
    let wit_component::DecodedWasm::Component(resolve, world) = wit_component::decode(component).unwrap() else {
        panic!("expected a component")
    };
    let mut printer = wit_component::WitPrinter::default();
    printer.print(&resolve, resolve.worlds[world].package.unwrap(), &[]).unwrap();
    printer.output.to_string()
}

/* === Tests === */

#[tokio::test]
async fn component_authoring_preserves_lifecycle_and_child_handles() {
    // Compare the Same SDK World Across Two Authored Catalogs
    let sandbox = super::support::component("harmos-sandbox-guest");
    let schematic = super::support::component("schematic-guest");
    assert_eq!(world(&sandbox), world(&schematic));
    let example = Session::new(&schematic).await;
    assert_eq!(example.metadata.id, "schematic_tools");

    // Register Inert Metadata Before Initializing Any Authored State
    let mut session = Session::new(&sandbox).await;
    assert!(session.store.data().calls.is_empty());
    let metadata = session.metadata.clone();
    assert_eq!(metadata.id, "sandbox_guest");
    let assignments = metadata
        .requirements
        .iter()
        .map(|required| host::AssignedOperation {
            operation: format!(
                "{}.{}",
                if required.id == "provider.echo" {
                    "provider"
                } else if required.id == "example.plugin.host.v1.Progress/Publish" {
                    "host"
                } else {
                    "sandbox_guest"
                },
                required.id
            ),
            fingerprint: required.fingerprint.clone(),
            generation: 1,
        })
        .collect();
    let initialize = v1::GuestInitialize {
        configuration: b"\"submit\"".to_vec(),
        generation_context: vec![2; 32],
        assignments: Some(host::OperationAssignments { operations: assignments }),
    };
    assert!(
        matches!(session.call("harmos.initialize", 1, initialize.encode_to_vec()).await, response::Result::Payload(payload) if payload.is_empty())
    );

    // Observe Every Submitted-Child Control on the Actual Component Import
    assert!(session.store.data().controls.contains(&v1::GuestChildAction::Status));
    assert!(session.store.data().controls.contains(&v1::GuestChildAction::Join));
    assert!(session.store.data().controls.contains(&v1::GuestChildAction::Cancel));
    assert!(session.store.data().controls.contains(&v1::GuestChildAction::Release));
    assert!(session.store.data().controls.contains(&v1::GuestChildAction::Detach));

    // Run a Unary Request and Preserve Its Structured Refusal Without Losing the Instance
    let request = EchoOperation { say: "hello".into() }.encode_to_vec();
    let answer = session.call("probe.echo", 2, request).await;
    assert!(matches!(answer, response::Result::Payload(payload) if Echoed::decode(payload.as_slice()).unwrap().said == "heard hello"));
    // Private Dispatch Never Acquires an Assignment or Crosses the Component Import.
    assert!(!metadata.catalog.as_ref().unwrap().operations.iter().any(|operation| operation.id == "probe.local"));
    let imported = session.store.data().calls.len();
    let answer = session.call("probe.echo", 12, EchoOperation { say: "local:inside".into() }.encode_to_vec()).await;
    assert!(matches!(answer, response::Result::Payload(payload) if Echoed::decode(payload.as_slice()).unwrap().said == "inside"));
    assert_eq!(session.store.data().calls.len(), imported);
    // Only the Initialization Context Carries Retained Generation Authority
    session.call("probe.echo", 10, EchoOperation { say: "dependency:finite".into() }.encode_to_vec()).await;
    assert_eq!(session.store.data().generations.last(), Some(&0));
    session.call("probe.echo", 11, EchoOperation { say: "saved-dependency:retained".into() }.encode_to_vec()).await;
    assert_eq!(session.store.data().generations.last(), Some(&32));
    let refused = session.call("probe.decline", 3, Vec::new()).await;
    assert!(
        matches!(refused, response::Result::Failure(failure) if failure.code == "7" && failure.classification == v1::Classification::Permanent as i32)
    );

    // Retain Capacity Across Prepared Phases and Release the Original Slots Idempotently
    assert!(matches!(session.prepare("probe.echo", 4, Vec::new()).await, response::Result::Payload(_)));
    assert!(matches!(session.prepare("probe.echo", 5, Vec::new()).await, response::Result::Payload(_)));
    assert!(matches!(session.prepare("probe.echo", 6, Vec::new()).await, response::Result::Failure(_)));
    session.phase(guest_command::Phase::Finalize(4)).await;
    session.phase(guest_command::Phase::Finalize(4)).await;
    assert!(matches!(session.prepare("probe.echo", 6, Vec::new()).await, response::Result::Payload(_)));
    session.phase(guest_command::Phase::Finalize(5)).await;
    session.phase(guest_command::Phase::Finalize(6)).await;

    // Reject a Malformed Request Before Running Its Handler
    assert!(matches!(session.prepare("probe.echo", 7, vec![255]).await, response::Result::Failure(_)));

    // Shut Down and Refuse Work After the Lifecycle Ends
    assert!(matches!(session.call("harmos.shutdown", 8, Vec::new()).await, response::Result::Payload(_)));
    assert!(matches!(session.prepare("probe.echo", 9, Vec::new()).await, response::Result::Failure(_)));
}
