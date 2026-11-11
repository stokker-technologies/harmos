/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Descriptor Fingerprint Contract Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;
use sha2::{Digest, Sha256};

// Local Imports
use super::support;
use harmos_protocol::{schema, v1};

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

#[test]
fn fingerprints_follow_the_message_graph_instead_of_catalog_composition() {
    // Compute A Consumer's Fingerprint From Its Own Definitions
    let types = support::types();
    let fingerprint = schema::fingerprint("fixture.echo", &types, "fixture.Value", "fixture.Value").unwrap();

    // Preserve The Original Minimal Fixture Hash Formula
    let minimal = schema::message_types(&types, "fixture.Value").unwrap();
    let mut bytes = b"fixture.echo\0".to_vec();
    bytes.extend(minimal.encode_to_vec());
    bytes.extend(minimal.encode_to_vec());
    assert_eq!(fingerprint, Sha256::digest(bytes).to_vec());
    assert_eq!(minimal.file[0].message_type.len(), 1);

    // Admit The Same Contract From A Merged Catalog With Compiler Metadata
    let mut merged = types.clone();
    merged.file[0].message_type.reverse();
    merged.file[0].source_code_info = Some(Default::default());
    merged.file[0].options = Some(prost_types::FileOptions {
        java_package: Some("compiler.generated".into()),
        ..Default::default()
    });
    merged.file[0].message_type[1].field[0].json_name = Some("value".into());
    assert_eq!(fingerprint, schema::fingerprint("fixture.echo", &merged, ".fixture.Value", "fixture.Value").unwrap());

    // Preserve Message Options In The Fingerprint
    merged.file[0].message_type[1].options = Some(prost_types::MessageOptions {
        deprecated: Some(true),
        ..Default::default()
    });
    assert_ne!(fingerprint, schema::fingerprint("fixture.echo", &merged, "fixture.Value", "fixture.Value").unwrap());
    merged.file[0].message_type[1].options = None;

    // Detect A Changed Wire Definition Or Operation Identity
    merged.file[0].message_type[1].field[0].r#type = Some(9);
    assert_ne!(fingerprint, schema::fingerprint("fixture.echo", &merged, "fixture.Value", "fixture.Value").unwrap());
    assert_ne!(fingerprint, schema::fingerprint("fixture.other", &types, "fixture.Value", "fixture.Value").unwrap());

    // Traverse Recursive And Imported Types Without Duplicating Definitions
    let recursive = schema::message_types(&types, "fixture.Node").unwrap();
    assert_eq!(recursive.file.len(), 2);
    assert_eq!(recursive.file[0].message_type.len(), 2);
    assert_eq!(recursive.file[0].message_type[0].name.as_deref(), Some("Node"));
    assert_eq!(recursive.file[1].enum_type[0].name.as_deref(), Some("Status"));
    let mut reordered = types.clone();
    reordered.file.reverse();
    assert_eq!(recursive, schema::message_types(&reordered, "fixture.Node").unwrap());

    // Reject Missing References And Ambiguous Schema Identity
    let mut broken = types.clone();
    broken.file[0].message_type[1].field[0].type_name = Some(".fixture.Missing".into());
    assert!(schema::message_types(&broken, "fixture.Node").is_err());
    broken.file.push(types.file[0].clone());
    assert!(schema::message_types(&broken, "fixture.Value").is_err());

    // The Catalog Digest Covers Changes And Ignores Its Own Advertised Value
    let mut catalog = v1::Catalog { types: Some(types), ..Default::default() };
    let digest = schema::catalog_digest(&catalog);
    catalog.digest = digest.clone();
    assert_eq!(schema::catalog_digest(&catalog), digest);
    catalog.types = Some(merged);
    assert_ne!(schema::catalog_digest(&catalog), digest);
}
