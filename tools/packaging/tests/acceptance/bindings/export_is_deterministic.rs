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
fn export_is_deterministic() {
    let artifacts = Artifacts::new(&contracts()).unwrap();
    let target = tempfile::tempdir().unwrap();
    let first = target.path().join("first");
    let repeat = target.path().join("repeat");
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_alpha"),
            protoc: &protoc(),
        },
        &first,
    )
    .unwrap();
    python::generate(
        python::Catalog {
            contracts: &artifacts.operation_contracts().unwrap(),
            settings: &settings("bindings_alpha"),
            protoc: &protoc(),
        },
        &repeat,
    )
    .unwrap();

    assert_eq!(files(&first), files(&repeat));
}
