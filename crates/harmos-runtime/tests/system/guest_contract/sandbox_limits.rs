/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Enforce Guest Sandbox Bounds

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{contract::*, support};
use harmos::OperationRequest as _;
use prost::Message as _;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn guest_sandbox_preserves_fuel_memory_and_trap_causes() {
    let _report = crate::report::system();
    // Enforce Each Sandbox Failure Against a Fresh Activation
    for (operation, payload, fuel) in [
        (SpinOperation::ID, SpinOperation {}.encode_to_vec(), true),
        (FailOperation::ID, FailOperation {}.encode_to_vec(), false),
        (EchoOperation::ID, EchoOperation { say: "grow".into() }.encode_to_vec(), false),
    ] {
        let (clock, _) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
        let lane = harmos_runtime::operations::Lane::default();
        let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
        let component = support::sandbox();
        let restricted = harmos_runtime::guest::Host::with_settings(
            &runtime,
            harmos_runtime::config::PluginGuestSettings {
                component_bytes: std::num::NonZeroUsize::new(component.len() - 1).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(restricted.load(&component), Err(harmos_runtime::guest::Error::Oversized { .. })));
        assert!(harmos_runtime::guest::Host::new(&runtime).unwrap().load(&component).is_err(), "a stream needs a consumer-owned window");
        if fuel {
            let restricted = harmos_runtime::guest::Host::with_settings(
                &runtime,
                harmos_runtime::config::PluginGuestSettings {
                    memory_bytes: std::num::NonZeroUsize::new(1).unwrap(),
                    stream_window: std::num::NonZeroUsize::new(4),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(restricted.load(&component).unwrap().register().await.is_err());
        }
        let host = support::host(&runtime);
        runtime.plugin.register_guest(host.load(&component).unwrap(), support::settings("normal"), lane).await.unwrap();
        runtime.plugin.initialize().await.unwrap();
        let failed = runtime.operations.call_encoded(operation, payload).await.unwrap_err();
        let harmos_runtime::operations::ComponentError::Transport(harmos_runtime::guest::Error::Execution { source }) =
            support::component_error(&failed)
        else {
            panic!("the original execution failure was lost: {failed:?}")
        };
        let trap = source.downcast_ref::<wasmtime::Trap>();
        if fuel {
            assert_eq!(trap, Some(&wasmtime::Trap::OutOfFuel));
        } else if operation == FailOperation::ID {
            assert!(trap.is_some());
            assert_ne!(trap, Some(&wasmtime::Trap::OutOfFuel));
        } else {
            assert!(trap.is_none(), "the store limiter returns its original host error");
        }
        assert!(support::call(&runtime.operations, EchoOperation { say: "fenced".into() }).await.is_err());
        if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
            let harmos_runtime::Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
            let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
            assert_eq!(failures.len(), 1);
        }
        runtime.plugin.stop("sandbox_guest").await.unwrap();
        let retired = runtime.plugin.status("sandbox_guest").unwrap();
        assert!(!retired.cleanup_pending);
        assert!(retired.last_failure.is_some());
        runtime.shutdown().await.unwrap();
    }

    // Bound Suspended Work Using the Host's Virtual Clock
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = harmos_runtime::operations::Lane::default();
    let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
    let host = support::host(&runtime);
    runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), support::settings("normal"), lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let invocation = runtime.operations.submit_encoded(WaitOperation::ID, WaitOperation {}.encode_to_vec()).unwrap();
    let id = invocation.id();
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(invocation, drive);
    assert!(matches!(answer, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut { .. }))));
    if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
        let harmos_runtime::Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
        let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
        assert_eq!(failures.len(), 1);
    }
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    let retired = runtime.plugin.status("sandbox_guest").unwrap();
    assert!(!retired.cleanup_pending);
    assert!(retired.last_failure.is_some());
    runtime.shutdown().await.unwrap();
}
