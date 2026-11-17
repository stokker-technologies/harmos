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

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, SystemTime},
};

// Third Party Imports
use prost::Message;
use wasmtime::Trap;

// Local Imports
use super::{
    contract::{EchoOperation, FailOperation, SpinOperation, WaitOperation},
    support,
};
use harmos::OperationRequest;
use harmos_runtime::{
    Error, Refusal,
    clock::Clock,
    config::PluginGuestSettings,
    guest::{self, Host, PluginFailure},
    operations::{ComponentError, Lane},
};

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

    // Compile Once Per Host Policy and Reject Invalid Bounds
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = Lane::default();
    let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
    let component = support::sandbox();
    let restricted = Host::with_settings(
        &runtime,
        PluginGuestSettings {
            component_bytes: NonZeroUsize::new(component.len() - 1).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(restricted.load(&component), Err(guest::Error::Oversized { .. })));
    assert!(Host::new(&runtime).unwrap().load(&component).is_err(), "a stream needs a consumer-owned window");
    let restricted = Host::with_settings(
        &runtime,
        PluginGuestSettings {
            memory_bytes: NonZeroUsize::new(1).unwrap(),
            stream_window: NonZeroUsize::new(4),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(restricted.load(&component).unwrap().register().await.is_err());

    // Activate the Compiled Component
    let host = support::host(&runtime);
    let loaded = host.load(&component).unwrap();
    runtime.plugin.register_guest(loaded, support::settings("normal"), lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();

    // Enforce Each Sandbox Failure Against a Fresh Activation
    for (operation, payload, fuel) in [
        (SpinOperation::ID, SpinOperation {}.encode_to_vec(), true),
        (FailOperation::ID, FailOperation {}.encode_to_vec(), false),
        (EchoOperation::ID, EchoOperation { say: "grow".into() }.encode_to_vec(), false),
    ] {
        let failed = runtime.operations.call_encoded(operation, payload).await.unwrap_err();
        let ComponentError::Transport(guest::Error::Execution { source }) = support::component_error(&failed) else {
            panic!("the original execution failure was lost: {failed:?}")
        };
        let trap = source.downcast_ref::<Trap>();
        if fuel {
            assert_eq!(trap, Some(&Trap::OutOfFuel));
        } else if operation == FailOperation::ID {
            assert!(trap.is_some());
            assert_ne!(trap, Some(&Trap::OutOfFuel));
        } else {
            assert!(trap.is_none(), "the store limiter returns its original host error");
        }
        assert!(support::call(&runtime.operations, EchoOperation { say: "fenced".into() }).await.is_err());
        if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
            let Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
            let PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
            assert_eq!(failures.len(), 1);
        }
        runtime.plugin.stop("sandbox_guest").await.unwrap();
        let retired = runtime.plugin.status("sandbox_guest").unwrap();
        assert!(!retired.cleanup_pending);
        assert!(retired.last_failure.is_some());

        // Reload the Compiled Component Into a Fresh Store
        runtime.plugin.reload("sandbox_guest").await.unwrap();
    }

    // Bound Suspended Work Using the Host's Virtual Clock
    let invocation = runtime.operations.submit_encoded(WaitOperation::ID, WaitOperation {}.encode_to_vec()).unwrap();
    let id = invocation.id();
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(invocation, drive);
    assert!(matches!(answer, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));
    if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
        let Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
        let PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
        assert_eq!(failures.len(), 1);
    }
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    let retired = runtime.plugin.status("sandbox_guest").unwrap();
    assert!(!retired.cleanup_pending);
    assert!(retired.last_failure.is_some());
    runtime.stop().await.unwrap();
}
