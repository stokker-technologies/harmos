/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export Generated Python Packages and Consume Them from the Repository
// Interpreter. Prerequisite: `uv sync --locked --project tools/python --group test`
// provisions the root `.venv` with protobuf, build and ty.

/* === Imports === */

use super::*;

/* === Tests === */

#[test]
fn exported_bindings_support_every_consumer() {
    // Export Both Producers Once
    let exported = export();
    let arguments = [exported.target.path(), &exported.alpha, &exported.beta];

    // Rust Encodes, Python Decodes and Re-Encodes, Rust Decodes the Same Value
    let value = Payload {
        text: "line\n∞".into(),
        optional: Some(u32::MAX),
        values: vec![i64::MIN, -1, 0, i64::MAX],
    };
    let encoded = {
        let mut bytes = Vec::new();
        grpc::encode(&value, &mut bytes).unwrap();
        bytes
    };

    for echoed in both("wire", &exported, &encoded) {
        let decoded = grpc::decode::<Payload>(DecodeInput::new(&echoed[..])).unwrap();
        assert_eq!(decoded.text, value.text);
        assert_eq!(decoded.optional, value.optional);
        assert_eq!(decoded.values, value.values);
    }

    // Inspect Descriptors In Both Protobuf Runtimes
    both("descriptors", &exported, &[]);

    // Import Both Producers Without Descriptor Collisions
    scene("isolation", &arguments, false, &[]);

    // Type Check Generated Stubs
    scene("typing", &arguments, false, &[]);

    // Install Both Packages Offline
    scene("install", &arguments, false, &[]);

    // Project Dynamic Contracts In Both Protobuf Runtimes
    both("dynamic", &exported, &[]);

    // identity::dotted Defines Operation Segments; QualifiedId Requires An Owner.
    // Rust Has No Length Ceiling; Python Retains Its Existing 256-Byte Bound.
    let cases = [
        ("a".to_owned(), true),
        ("read_2".to_owned(), true),
        ("a0.b_2".to_owned(), true),
        ("editor.appearance.get_appearance".to_owned(), true),
        ("host.storage.read_file".to_owned(), true),
        ("a".repeat(255), true),
        ("a".repeat(256), true),
        ("a".repeat(257), true),
        (format!("host.{}", "a".repeat(251)), true),
        (format!("host.{}", "a".repeat(252)), true),
        ("".to_owned(), false),
        (".read".to_owned(), false),
        ("host.".to_owned(), false),
        ("host..read".to_owned(), false),
        ("Host.read".to_owned(), false),
        ("host.Read".to_owned(), false),
        ("2host.read".to_owned(), false),
        ("host.2read".to_owned(), false),
        ("host.read.2file".to_owned(), false),
        ("_host.read".to_owned(), false),
        ("host._read".to_owned(), false),
        ("host-name.read".to_owned(), false),
        ("host.read-file".to_owned(), false),
        ("host/read".to_owned(), false),
        ("host read".to_owned(), false),
        ("host.é".to_owned(), false),
        ("höst.read".to_owned(), false),
        ("host.read\n".to_owned(), false),
        ("host.read\0".to_owned(), false),
    ];
    let template = harmos_operations::operations::Contracts::from_tree(ROUTES_CATALOG.declarations).unwrap()[0].operation().clone();
    for (value, valid) in &cases {
        let parsed = Identifier::new(value.clone()).is_ok_and(|id| {
            let mut operation = template.clone();
            operation.id = id;
            operation.validate().is_ok()
        });
        assert_eq!(parsed, *valid, "operation {value:?}");
        assert_eq!(QualifiedId::parse(value).is_ok(), *valid && value.contains('.'), "qualified {value:?}");
    }

    both("identities", &exported, &serde_json::to_vec(&cases).unwrap());

    let cases: Vec<(String, bool)> =
        serde_json::from_str(include_str!("../../../../../crates/harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
    for (owner, valid) in &cases {
        assert_eq!(harmos_domain::identity::segment(owner), *valid, "{owner:?}");
    }
    both("owners", &exported, &serde_json::to_vec(&cases).unwrap());

    // Reject Invalid Dynamic Contract Graphs
    scene("refusals", &arguments, false, &[]);
}
