/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Untrusted Catalog Admission Story

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Remote};
use harmos_protocol::model;
use harmos_protocol::schema;
use harmos_runtime::plugin::grpc::{Error, Placement};

/* === Tests === */

#[tokio::test]
async fn host_refuses_tampered_catalogs_before_activation() {
    // Start A Host And Remote Endpoint
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;

    // Private and Unspecified Visibility Never Enter the Wire Registry.
    let mut catalog = support::catalog();
    catalog.operations[0].visibility = model::Visibility::Private as i32;
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));
    assert!(runtime.operations.definition("fixture.echo").is_err());

    let mut catalog = support::catalog();
    catalog.operations[0].visibility = model::Visibility::Unspecified as i32;
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));
    assert!(runtime.operations.definition("fixture.echo").is_err());

    // Refuse A Catalog Whose Bytes Do Not Match Its Advertised Digest
    remote.declarations(model::Declarations::default());
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Refuse A Rehashed Catalog Built Against A Different Operation Schema
    let mut catalog = support::catalog();
    catalog.operations[0].fingerprint[0] ^= 1;
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Refuse A Correctly Hashed Catalog With An Unresolved Operation Schema
    let mut catalog = model::Declarations {
        operations: vec![model::OperationContract {
            id: "fixture.echo".into(),
            request: "fixture.Missing".into(),
            response: "fixture.Missing".into(),
            visibility: model::Visibility::Public as i32,
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));

    // Refuse An Invalid Cadence Without Leaving Operations Admitted
    let mut catalog = support::catalog();
    catalog.streams.push(model::StreamDeclaration {
        id: "fixture.ticks".into(),
        row: "fixture.Value".into(),
        interval: Some(prost_types::Duration { seconds: 0, nanos: 0 }),
        ..Default::default()
    });
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));
    assert!(runtime.operations.definition("fixture.echo").is_err());

    // Refuse An Unresolved Interval Setting
    let mut catalog = support::catalog();
    catalog.streams.push(model::StreamDeclaration {
        id: "fixture.ticks".into(),
        row: "fixture.Value".into(),
        interval_setting: "fixture.Settings.interval".into(),
        ..Default::default()
    });
    catalog.digest = schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    assert!(matches!(result, Err(Error::Invalid(_))));
    assert!(runtime.operations.definition("fixture.echo").is_err());

    // Admit The Repaired Endpoint And Cleanly Remove It
    let catalog = support::catalog();
    remote.declarations(catalog.clone());
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    assert_eq!(attached.activation().unwrap().description.declarations, Some(catalog));
    attached.stop("Catalog Verified").await.unwrap();
    runtime.shutdown().await.unwrap();
}
