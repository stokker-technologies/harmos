/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../../../harmos-guest/tests/fixtures/sandbox/src/contract.rs"]
mod sandbox;

mod dependencies;
mod lifecycle;
mod limits;
mod operations;
mod publications;
use schematic_domain as schematic;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::OnceLock;
// Third Party Imports

// Local Imports
use super::support;
use harmos_runtime::Runtime;
use harmos_runtime::guest::Error;
use schematic::model::Point;
use schematic_operations::schematic::propose;
use support::Bank;
use support::fixtures::component as load;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

use support::Lanes as GuestLanes;

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> Runtime<Bank> {
    Runtime::builder(support::Ledger::default(), ())
        .register(GuestLanes::catalog())
        .register(support::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
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
    COMPONENT.get_or_init(|| load("harmos-sandbox-guest")).clone()
}

// A Route From The Origin To `(x, 4)` Turns At `(x, 0)`
fn route(x: i32) -> propose::ProposeRoute {
    propose::ProposeRoute {
        from: Point { x: 0, y: 0 },
        to: Point { x, y: 4 },
    }
}

fn corner(x: i32) -> [Point; 3] {
    [Point { x: 0, y: 0 }, Point { x, y: 0 }, Point { x, y: 4 }]
}

fn component_error(error: &harmos_runtime::Error) -> &harmos_runtime::operations::ComponentError {
    let harmos_runtime::Error::OperationFailed { source, .. } = error else {
        panic!("expected the original handler failure, got {error:?}");
    };
    source.downcast_ref().expect("component handler source is retained")
}

fn execution_error(error: &harmos_runtime::Error) -> &wasmtime::Error {
    let harmos_runtime::operations::ComponentError::Transport(Error::Execution { source }) = component_error(error) else {
        panic!("expected the original guest execution failure, got {error:?}");
    };
    source
}

// A Failed Generation Reports Its SDK Shutdown Refusal Once, Then Retires
async fn retire_failed_guest<L: harmos_runtime::plugin::PluginLifecycle>(
    manager: &mut harmos_runtime::plugin::PluginManager<L::Guest, L::Sidecar, L::Embedded>, lifecycle: &mut L,
) {
    let harmos_runtime::Error::PluginLifecycle { source } = manager.shutdown(lifecycle).await.unwrap_err() else {
        panic!("the original failed generation must report its SDK shutdown refusal")
    };
    let harmos_runtime::plugin::PluginFailure::Cleanup(failures) = *source else {
        panic!("the failed generation retains its original physical cleanup owner")
    };
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].artifact, "sandbox_guest");
    let harmos_runtime::Error::TaskFailed { domain, source } = &failures[0].error else {
        panic!("the manager preserves its original cleanup failure");
    };
    assert_eq!(*domain, "plugin cleanup");
    let original = source.downcast_ref::<harmos_runtime::Error>().expect("the original SDK error remains available");
    assert!(matches!(component_error(original), harmos_runtime::operations::ComponentError::Unavailable));
    manager.shutdown(lifecycle).await.unwrap();
}

/* === Tests === */
