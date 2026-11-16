/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Plugin's Startup Deadline Is the plugins.load Operation's Deadline

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{
    invoke_support::pending,
    support::{self, Manual, Remote},
};
use harmos_runtime::{
    clock::{Clock, Source as _},
    operations::{NativeOperation, OperationPolicy},
    plugin::grpc::{Error, Load, LoadOptions, Placement},
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
async fn a_slow_plugin_misses_the_load_deadline_and_a_per_call_override_lets_it_start() {
    // A Remote Plugin That Describes Itself Only When the Story Releases It, on a Controlled Host Clock.
    let source = Manual::new();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = Remote::start().await;
    let policy = <Load as NativeOperation>::POLICY;

    // Under the Operation's Own Policy, the Load Ends at Its Deadline and Names the Plugin.
    let (mut entered, _held) = remote.delay_description();
    let deadline = source.read().instant + policy.timeout;
    let attaching = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings());
    tokio::pin!(attaching);
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Load Only Once the Plugin Describes Itself"),
        entered = &mut entered => entered.unwrap(),
    }
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Keep Loading Until the Deadline"),
        () = source.registered_at(deadline) => {},
    }
    source.advance_by(policy.timeout);
    let refused = attaching.await;
    assert!(
        matches!(
            &refused,
            Err(Error::LoadDeadline { plugin, timeout, configured_by: "operation policy.timeout" })
                if *plugin == remote.address && *timeout == policy.timeout
        ),
        "Fail the Load at the Operation's Deadline: {:?}",
        refused.err()
    );

    // A Longer Deadline for This One Load Lets the Same Slow Start Finish Past the Policy's Deadline.
    let (mut entered, release) = remote.delay_description();
    let overridden = OperationPolicy {
        timeout: policy.timeout * 4,
        attempt_timeout: policy.timeout * 4,
        ..policy
    };
    let options = LoadOptions { policy: Some(overridden) };
    let attaching = runtime.attach_grpc_with(Placement::Remote(remote.address.clone()), support::settings(), options);
    tokio::pin!(attaching);
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Load Only Once the Plugin Describes Itself"),
        entered = &mut entered => entered.unwrap(),
    }
    source.advance_by(policy.timeout * 2);
    assert!(pending(attaching.as_mut()).await, "Keep Loading Past the Policy's Deadline Under the Override");
    release.send(()).unwrap();
    let attached = attaching.await.unwrap();
    assert!(attached.activation().is_some(), "Start the Plugin Within the Overridden Deadline");
    attached.stop("Story Complete").await.unwrap();
    runtime.shutdown().await.unwrap();
}
