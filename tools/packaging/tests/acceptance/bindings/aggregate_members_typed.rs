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
fn aggregate_members_typed() {
    // Generate a Public Consumer Package Carrying Typed Aggregates
    let template = harmos_operations::operations::Contracts::from_tree(ROUTES_CATALOG.declarations).unwrap()[0].clone();
    let schema = Schema::from_declaration(Summary::DEFINITION, &SchemaSettings::default()).unwrap();
    let summary = contract(&template, "fixture_aggregate", "Aggregate", schema);
    let artifacts = Artifacts::new([&summary]).unwrap();
    let target = tempfile::tempdir().unwrap();
    let package = target.path().join("package");
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_aggregate"),
            protoc: &protoc(),
        },
        &package,
    )
    .unwrap();
    // Python Selects the Percentile Member and Its Rank as Typed Fields
    let value = Summary {
        aggregate: Aggregate::Percentile(95),
        aggregates: vec![Aggregate::Count, Aggregate::Percentile(99)],
    };
    let source = package.join("src");
    let output = scene("aggregate", &[&source], false, &{
        let mut bytes = Vec::new();
        grpc::encode(&value, &mut bytes).unwrap();
        bytes
    });
    assert!(grpc::decode::<Summary>(DecodeInput::new(&output[..])).unwrap() == value);
}
