/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/mod.rs"]
mod support;

#[path = "../../../harmos-guest/tests/fixtures/sandbox/src/contract.rs"]
mod sandbox;

mod dependencies;
mod lifecycle;
mod limits;
mod operations;
mod publications;
mod schematic;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::OnceLock;
// Third Party Imports

// Local Imports
use harmos::Runtime;
use harmos::guest::Error;
use schematic::model::Point;
use schematic::operations::propose;
use support::Bank;
use support::fixtures::component as load;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::lanes]
enum GuestLanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<Bank> {
    Runtime::builder(support::Ledger::default(), ())
        .register(GuestLanes::catalog())
        .register(support::catalog())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .expect("an empty ledger has nothing to recover")
}

fn schematic() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT.get_or_init(|| load("schematic-guest")).clone()
}

fn sandbox() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("harmos-sandbox-guest"))
        .clone()
}

// A Route From The Origin To `(x, 4)` Turns At `(x, 0)`
fn route(x: i32) -> propose::Request {
    propose::Request {
        from: Point { x: 0, y: 0 },
        to: Point { x, y: 4 },
    }
}

fn corner(x: i32) -> [Point; 3] {
    [Point { x: 0, y: 0 }, Point { x, y: 0 }, Point { x, y: 4 }]
}

fn component_error(error: &harmos::Error) -> &harmos::operations::ComponentError {
    let harmos::Error::OperationFailed { source, .. } = error else {
        panic!("expected the original handler failure, got {error:?}");
    };
    source
        .downcast_ref()
        .expect("component handler source is retained")
}

fn execution_error(error: &harmos::Error) -> &wasmtime::Error {
    let harmos::operations::ComponentError::Transport(Error::Execution { source }) =
        component_error(error)
    else {
        panic!("expected the original guest execution failure, got {error:?}");
    };
    source
}

// A Failed Generation Reports Its SDK Shutdown Refusal Once, Then Retires
async fn retire_failed_guest<L: harmos::plugin::PluginLifecycle>(
    manager: &mut harmos::plugin::PluginManager<L::Guest, L::Sidecar, L::Embedded>,
    lifecycle: &mut L,
) {
    let harmos::Error::PluginLifecycle { source } = manager.shutdown(lifecycle).await.unwrap_err()
    else {
        panic!("the original failed generation must report its SDK shutdown refusal")
    };
    let harmos::plugin::PluginFailure::Cleanup(failures) = *source else {
        panic!("the failed generation retains its original physical cleanup owner")
    };
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].artifact, "sandbox-guest");
    let harmos::Error::TaskFailed { domain, source } = &failures[0].error else {
        panic!("the manager preserves its original cleanup failure");
    };
    assert_eq!(*domain, "plugin cleanup");
    let original = source
        .downcast_ref::<harmos::Error>()
        .expect("the original SDK error remains available");
    assert!(matches!(
        component_error(original),
        harmos::operations::ComponentError::Unavailable
    ));
    manager.shutdown(lifecycle).await.unwrap();
}

/* === Tests === */
