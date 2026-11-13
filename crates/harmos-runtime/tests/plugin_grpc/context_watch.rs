/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Observes a Running Invocation Through Its Host Context.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;

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
async fn context_watch_delivers_running_and_settled_states_on_the_parent_carrier() {
    let host = HostFixture::new().await;
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let worker = host.sdk("caller").await;
    let _worker = host.runtime.attach_grpc(Placement::Remote(worker.address.clone()), support::settings()).await.unwrap();
    let watcher = host.sdk("watch").await;
    let _watcher = host.runtime.attach_grpc(Placement::Remote(watcher.address.clone()), support::settings()).await.unwrap();

    // A Child Call Is the Awaited Evidence That the Target Is Running.
    let target = host.runtime.operations.submit_encoded("fixture.wait", payload(b"target")).unwrap();
    host.signals.entered.notified().await;
    let watching = host.runtime.operations.submit_encoded("fixture.watch", payload(&target.id().value().to_le_bytes())).unwrap();
    // The Watcher Acknowledges Its First Received State Through a Host Operation.
    host.signals.entered.notified().await;
    target.request_cancel();
    assert_eq!(target.await.unwrap(), payload(b"target"));
    let response = watching.await.unwrap();
    #[derive(prost::Message)]
    struct Value {
        #[prost(bytes = "vec", tag = "1")]
        value: Vec<u8>,
    }
    let response = <Value as prost::Message>::decode(response.as_slice()).unwrap();
    let phases = String::from_utf8(response.value).unwrap();
    assert!(phases.starts_with("running|"), "{phases}");
    assert!(phases.ends_with("settled|"), "{phases}");
    host.runtime.shutdown().await.unwrap();
}
