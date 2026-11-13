/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Settle Guest Cancellation

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
async fn guest_cancellation_keeps_the_cooperative_final_response() {
    let _report = crate::report::system();
    // Compare Cooperative Settlement With Each Noncooperative Handler
    for mode in 0..3 {
        let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
        let lane = harmos_runtime::operations::Lane::default();
        let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
        let host = support::host(&runtime);
        runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), support::settings("normal"), lane).await.unwrap();
        runtime.plugin.initialize().await.unwrap();

        // Observe Handler Entry Through Another Unary Operation
        let mut invocation = runtime.operations.submit_encoded(FinishOperation::ID, FinishOperation { mode }.encode_to_vec()).unwrap();
        loop {
            let counts = tokio::select! {
                result = &mut invocation => panic!("the handler completed before cancellation: {result:?}"),
                counts = support::call(&runtime.operations, CountsOperation { value: String::new() }) => counts.unwrap(),
            };
            if counts.value == "1" {
                break;
            }
            tokio::task::yield_now().await;
        }

        // Cancel the Original Invocation and Drive Only Host Time
        let id = invocation.id();
        invocation.request_cancel();
        let drive = async { controller.advance_until(Duration::from_secs(5), || runtime.operations.status(id).is_none()).await.unwrap() };
        let (answer, ()) = tokio::join!(invocation, drive);
        if mode == 0 {
            assert_eq!(Value::decode(answer.unwrap().as_slice()).unwrap().value, "42");
            runtime.plugin.stop("sandbox_guest").await.unwrap();
        } else {
            assert!(matches!(answer, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
            if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
                let harmos_runtime::Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
                let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
                assert_eq!(failures.len(), 1);
            }
            runtime.plugin.stop("sandbox_guest").await.unwrap();
            let retired = runtime.plugin.status("sandbox_guest").unwrap();
            assert!(!retired.cleanup_pending);
            assert!(retired.last_failure.is_some());
        }
        runtime.shutdown().await.unwrap();
    }
}
