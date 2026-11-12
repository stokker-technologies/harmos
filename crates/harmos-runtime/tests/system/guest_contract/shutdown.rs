/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Shutdown Cleanup

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::support;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
async fn guest_shutdown_retains_the_original_cleanup_after_a_deadline() {
    let _report = crate::report::system();
    // Start a Guest Whose Authored Shutdown Does Not Cooperate
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = harmos_runtime::operations::Lane::default();
    let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
    let host = support::host(&runtime);
    let settings = harmos_runtime::config::PluginSettings {
        shutdown_timeout: Duration::from_millis(20),
        ..support::settings("shutdown-pending")
    };
    runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), settings, lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let original = runtime.plugin.status("sandbox_guest").unwrap();

    // Let the Outer Deadline Return While the Same Cleanup Remains Owned
    let done = AtomicBool::new(false);
    let stopping = async {
        let result = runtime.plugin.stop("sandbox_guest").await;
        done.store(true, Ordering::SeqCst);
        result
    };
    let drive = async { controller.advance_until(Duration::from_secs(2), || done.load(Ordering::SeqCst)).await.unwrap() };
    let (stopped, ()) = tokio::join!(stopping, drive);
    assert!(stopped.is_err());
    let failed = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(failed.generation, original.generation);
    assert_eq!(failed.attempts, original.attempts);
    assert!(failed.cleanup_pending);

    // Resume That Original Cleanup Until the Guest's Own Deadline Settles It
    let mut settled = false;
    loop {
        let done = AtomicBool::new(false);
        let stopping = async {
            let result = runtime.plugin.stop("sandbox_guest").await;
            done.store(true, Ordering::SeqCst);
            result
        };
        let drive = async { controller.advance_until(Duration::from_secs(2), || done.load(Ordering::SeqCst)).await.unwrap() };
        let (stopped, ()) = tokio::join!(stopping, drive);
        match stopped {
            Ok(()) => {
                assert!(settled, "the original guest deadline must settle its cleanup");
                break;
            }
            Err(harmos_runtime::Error::PluginLifecycle { source }) => {
                let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
                assert!(!settled);
                let [failure] = failures.as_slice() else { panic!("one original guest owns cleanup") };
                let harmos_runtime::Error::TaskFailed { source, .. } = &failure.error else { panic!("cleanup retains its original error") };
                match source.downcast_ref::<harmos_runtime::Error>().unwrap() {
                    harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut { .. }) => settled = true,
                    harmos_runtime::Error::NotReady { domain: "guest plugin shutdown timeout" } => {}
                    error => panic!("unexpected retained failure: {error:?}"),
                }
            }
            Err(error) => panic!("unexpected shutdown error: {error:?}"),
        }
    }
    assert_eq!(runtime.plugin.status("sandbox_guest").unwrap().attempts, original.attempts);
    runtime.shutdown().await.unwrap();
}
