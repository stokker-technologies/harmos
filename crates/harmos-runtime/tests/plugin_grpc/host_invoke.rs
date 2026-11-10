/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Invokes an SDK Plugin

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
async fn host_invokes_registered_plugin_operation() {
    // Connect An SDK Plugin
    let host = HostFixture::new().await;
    let sdk = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();

    // Invoke Its Private Operation As The Host
    let input = payload(b"echo");
    let response = host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap();
    assert_eq!(response, input);
    assert_eq!(host.runtime.operations.definition("fixture.echo").unwrap().visibility, harmos_runtime::operations::Visibility::Private);

    // Stop Removes Operations From The Registry
    attached.stop("Story Complete").await.unwrap();
    assert!(host.runtime.operations.definition("fixture.echo").is_err());
    host.runtime.shutdown().await.unwrap();
}
