/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Untrusted Catalog Admission Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Remote};
use harmos_protocol::{schema, v1};
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
async fn host_refuses_tampered_catalogs_before_activation() {
    // Start A Host And Remote Endpoint
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;

    // Refuse A Catalog Whose Bytes Do Not Match Its Advertised Digest
    remote.catalog(v1::Catalog::default());
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Refuse A Rehashed Catalog Built Against A Different Operation Schema
    let mut catalog = support::catalog();
    catalog.operations[0].fingerprint[0] ^= 1;
    catalog.digest = schema::catalog_digest(&catalog);
    remote.catalog(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Refuse A Correctly Hashed Catalog With An Unresolved Operation Schema
    let mut catalog = v1::Catalog {
        operations: vec![v1::OperationContract {
            id: "fixture.echo".into(),
            request: "fixture.Missing".into(),
            response: "fixture.Missing".into(),
            visibility: v1::Visibility::Public as i32,
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = schema::catalog_digest(&catalog);
    remote.catalog(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Admit The Repaired Endpoint And Cleanly Remove It
    let catalog = support::catalog();
    remote.catalog(catalog.clone());
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    assert_eq!(attached.activation().unwrap().description.catalog, Some(catalog));
    attached.stop("Catalog Verified").await.unwrap();
    runtime.shutdown().await.unwrap();
}
