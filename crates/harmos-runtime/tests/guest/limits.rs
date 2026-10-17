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

// Local Imports
use super::lifecycle::{Lifecycle, Manager};
use super::sandbox::{Decline, Echo, Fail, Spin, Wait};
use super::{component_error, execution_error, retire_failed_guest, runtime, schematic};
use harmos_runtime::guest::{Error, Host};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[tokio::test]
async fn host_resource_settings() {
    let runtime = runtime().await;
    let bytes = schematic();
    let settings = harmos_runtime::config::PluginGuestSettings {
        component_bytes: std::num::NonZeroUsize::new(bytes.len() - 1).unwrap(),
        ..Default::default()
    };
    let restricted = Host::with_settings(&runtime, settings).unwrap();
    assert!(
        matches!(restricted.load(&bytes), Err(Error::Oversized { limit }) if limit == bytes.len() - 1)
    );
    let settings = harmos_runtime::config::PluginGuestSettings {
        memory_bytes: std::num::NonZeroUsize::new(1).unwrap(),
        ..Default::default()
    };
    let restricted = Host::with_settings(&runtime, settings).unwrap();
    let refused = restricted.load(&bytes).unwrap().register().await;
    let error = match refused {
        Ok(_) => panic!("one byte cannot admit the component's initial memory"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("memory"), "{error}");
    let allowed = Host::new(&runtime)
        .unwrap()
        .load(&bytes)
        .unwrap()
        .register()
        .await
        .unwrap();
    drop(allowed);
    drop(restricted);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn memory_growth_bounded() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let refused = runtime
        .operations
        .call(Echo { say: "grow".into() })
        .await
        .unwrap_err();
    assert!(
        execution_error(&refused)
            .root_cause()
            .to_string()
            .starts_with("forcing trap when growing memory to "),
        "{refused}"
    );
    retire_failed_guest(&mut manager, &mut lifecycle).await;
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn fuel_retains_cause() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let spun = runtime
        .operations
        .call(Spin {})
        .await
        .unwrap_err();
    assert_eq!(
        execution_error(&spun).downcast_ref::<wasmtime::Trap>(),
        Some(&wasmtime::Trap::OutOfFuel)
    );
    retire_failed_guest(&mut manager, &mut lifecycle).await;
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn manager_bounds_wait() {
    let runtime = runtime().await;
    // The Embedded Provider Never Answers The Waiting Probe
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let waited = runtime
        .operations
        .call(Wait {})
        .await
        .unwrap_err();
    assert!(
        matches!(
            waited,
            harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut)
        ),
        "{waited:?}"
    );
    retire_failed_guest(&mut manager, &mut lifecycle).await;
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn trap_discards_instance() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let trapped = runtime
        .operations
        .call(Fail {})
        .await
        .unwrap_err();
    assert_ne!(
        execution_error(&trapped).downcast_ref::<wasmtime::Trap>(),
        Some(&wasmtime::Trap::OutOfFuel)
    );
    assert!(
        execution_error(&trapped)
            .downcast_ref::<wasmtime::Trap>()
            .is_some()
    );
    let after = runtime
        .operations
        .call(Echo {
            say: "anyone".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        component_error(&after),
        harmos_runtime::operations::ComponentError::Unavailable
    ));
    retire_failed_guest(&mut manager, &mut lifecycle).await;
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn refusal_keeps_classification() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let refused = runtime
        .operations
        .call(Decline {})
        .await
        .unwrap_err();
    let harmos_runtime::operations::ComponentError::Remote(failure) = component_error(&refused) else {
        panic!("expected classified refusal, got {refused:?}");
    };
    assert_eq!(failure.cause(), "the probe declines");
    let answered = runtime
        .operations
        .call(Echo { say: "two".into() })
        .await
        .unwrap();
    assert_eq!(answered.said, "two", "a declined call keeps the instance");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

/* === Tests === */
