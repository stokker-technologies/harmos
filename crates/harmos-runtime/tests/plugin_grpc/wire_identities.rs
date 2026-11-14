/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Numeric SDK Operation Crosses Catalog Admission and the Instance Carrier.

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
async fn numeric_plugin_operation_is_admitted_and_invoked() {
    // Admit an SDK Catalog With a Template-Local Numeric Binding.
    let host = HostFixture::new().await;
    let sdk = host.sdk("provider").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let operation =
        activation.description.declarations.as_ref().unwrap().operations.iter().find(|operation| operation.id == "fixture.step.0").unwrap();
    assert_eq!(operation.visibility(), harmos_protocol::v1::Visibility::Public);

    // Invoke Through the Host and Await the SDK Result.
    let input = payload(b"numeric step");
    assert_eq!(host.runtime.operations.call_encoded("fixture.step.0", input.clone()).await.unwrap(), input);

    // Retire the Original Activation.
    attached.stop("Numeric Step Complete").await.unwrap();
    assert!(host.runtime.operations.definition("fixture.step.0").is_err());
    host.runtime.shutdown().await.unwrap();
}
