/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// An Untrusted Endpoint Repairs Its Wire Identity Before Admission.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Remote};
use harmos_protocol::model;
use harmos_protocol::schema;
use harmos_runtime::plugin::grpc::{Error, Placement};

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
async fn malformed_and_reserved_plugin_identities_are_refused_before_activation() {
    // Keep One Endpoint Available While Its Catalog Is Repaired.
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;
    for id in [
        "example..step.0",
        "example.plugin.v1.Steps/",
        "harmos.plugin.v1.Instance/Invoke",
        "example.host.v1.Prompts/Create",
        "example.host.v1.Steps/Execute",
    ] {
        let mut catalog = support::catalog();
        let operation = &mut catalog.operations[0];
        operation.id = id.into();
        operation.fingerprint = schema::fingerprint(id, catalog.types.as_ref().unwrap(), &operation.request, &operation.response).unwrap();
        catalog.digest = schema::declarations_digest(&catalog);
        remote.declarations(catalog);

        // Refuse Even a Correct Descriptor, Fingerprint and Catalog Digest.
        let error = match runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await {
            Err(error) => error,
            Ok(_) => panic!("Register {id}: Invalid Identity Was Admitted"),
        };
        assert!(matches!(error, Error::Invalid(_)));
        let message = error.to_string();
        assert!(message.contains(id), "{message}");
        assert!(message.contains("Declare") || message.contains("Register"), "{message}");
        assert!(runtime.operations.definition(id).is_err());
    }

    // A Canonical Application Plugin Identity Can Be Admitted After Repair.
    let mut catalog = support::catalog();
    let operation = &mut catalog.operations[0];
    operation.id = "example.plugin.v1.Steps/Execute".into();
    operation.visibility = model::Visibility::Public as i32;
    operation.fingerprint =
        schema::fingerprint(&operation.id, catalog.types.as_ref().unwrap(), &operation.request, &operation.response).unwrap();
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    attached.stop("Identity Repaired").await.unwrap();
    runtime.shutdown().await.unwrap();
}
