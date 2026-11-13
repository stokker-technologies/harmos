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
fn export_refuses_invalid() {
    let contracts = harmos_operations::operations::Contracts::from_tree(ROUTES_CATALOG.declarations).unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let output = target.path().join("package");
    let refused = |settings: &Settings| {
        python::generate(
            python::Catalog {
                contracts: &artifacts.operation_contracts().unwrap(),
                settings,
                protoc: &protoc(),
            },
            &output,
        )
    };

    // Selections, Module Names and Versions Python Cannot Carry
    let mut settings = settings("bindings_alpha");
    settings.operations = vec!["missing.operation".into()];
    assert!(matches!(refused(&settings), Err(python::Error::Selection(_))));
    settings.operations = vec![contracts[0].operation().id.to_string(); 2];
    assert!(matches!(refused(&settings), Err(python::Error::Collision(_))));
    settings.operations.clear();
    settings.module = "class".into();
    assert!(matches!(refused(&settings), Err(python::Error::Settings(_))));
    settings.module = "bindings_alpha".into();
    settings.version = "1.0.0-preview.1".parse().unwrap();
    assert!(matches!(refused(&settings), Err(python::Error::Settings(_))));
    settings.version = "1.0.0".parse().unwrap();

    // Contracts Without gRPC Routes Or Whose Escaped Names Collide
    let unsupported = contracts[0].clone().with_grpc(None).unwrap();
    let unsupported = Artifacts::new([&unsupported]).unwrap();
    assert!(matches!(
        python::generate(
            python::Catalog {
                contracts: &unsupported.operation_contracts().unwrap(),
                settings: &settings,
                protoc: &protoc()
            },
            &output
        ),
        Err(python::Error::Unsupported(_))
    ));
    let mut collision = reserved();
    collision.definitions[0].push(Field {
        name: Cow::Borrowed("Clear_"),
        tag: 8,
        cardinality: Cardinality::Variant,
        kind: Kind::Message { definition: 1 },
    });
    let collision = contract(&contracts[0], "fixture_collision", "Collision", collision);
    assert!(Artifacts::new([&collision]).is_err());
    assert!(!output.exists());

    // An Existing Directory Is Never Overwritten
    fs::create_dir(&output).unwrap();
    fs::write(output.join("owned.txt"), "preserved").unwrap();
    assert!(matches!(refused(&settings), Err(python::Error::Exists(_))));
    assert_eq!(fs::read_to_string(output.join("owned.txt")).unwrap(), "preserved");
}
