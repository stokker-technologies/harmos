/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Matching Requirement Does Not Admit a Plugin to the Client Face.

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
async fn plugin_cannot_call_a_client_face_even_with_its_declared_contract() {
    let host = HostFixture::new().await;
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();
    let error = host.runtime.operations.call_encoded("fixture.forward", payload(b"client-only")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(call_error(&error).to_string().contains("harmos.v1.Time/Get"));
    host.runtime.shutdown().await.unwrap();
}
