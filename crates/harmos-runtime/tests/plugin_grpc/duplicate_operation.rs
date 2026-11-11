/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Duplicate Operation Admission Is Refused

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
async fn duplicate_operation_names_both_owners() {
    // Admit The Original SDK Plugin
    let host = HostFixture::new().await;
    let original = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(original.address.clone()), support::settings()).await.unwrap();

    // Refuse A Second Owner Of The Same Identifier
    let duplicate = host.sdk("duplicate").await;
    let error = host.runtime.attach_grpc(Placement::Remote(duplicate.address.clone()), support::settings()).await.err().unwrap();
    let message = error.to_string();
    assert!(message.contains("fixture.echo"));
    assert!(message.contains("caller"));
    assert!(message.contains("duplicate"));

    // Host Registration Cannot Replace The Plugin's Identifier Either
    use harmos_runtime::operations::Registration;
    let batch = Registration::new(harmos_runtime::operations::Lane::default()).operation::<Host>("fixture.echo").unwrap();
    let error = host.runtime.register_batch(batch).await.unwrap_err();
    assert!(error.to_string().contains("fixture.echo"));
    assert!(error.to_string().contains("caller"));
    assert!(error.to_string().contains("host"));

    // The Original Provider Still Answers
    let input = payload(b"original");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
