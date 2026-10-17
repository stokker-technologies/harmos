/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
// Exercise The Original Helper Without Selecting The Authoring Kit.
#[path = "../../src/kit/task.rs"]
mod observer;
#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod prepared_fixtures;
mod task;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

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
fn inert_native_metadata() {
    let root = tempfile::tempdir().unwrap();
    let artifact = root.path().join("plugin.data");
    // A Non-Executable Copy Suffices; The Plugin Lifecycle Panics If Entered.
    std::fs::write(
        &artifact,
        std::fs::read(prepared_fixtures::binary("harmos-sidecar-catalog-fixture")).unwrap(),
    )
    .unwrap();
    let bytes = std::fs::read(&artifact).unwrap();
    let declaration = harmos_sidecar::inventory::Declaration::inspect(
        &bytes,
        harmos_sidecar::inventory::Facility::Sidecar,
    )
    .unwrap();
    assert!(!declaration.contracts.provides.is_empty());
}
