/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs;
// Third Party Imports

// Local Imports
use crate::fixtures;

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
fn authoring_refusals_compile() {
    let harmos = fixtures::library("harmos");
    let harmos_sidecar = fixtures::library("harmos_sidecar");
    let serde = fixtures::library("serde");
    let dependencies = fixtures::dependencies();
    let fixtures = fixtures::manifest().join("tests/fixtures");
    let output = std::env::temp_dir().join(format!("harmos-stream-ui-{}", std::process::id()));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    fs::create_dir_all(&output).expect("the compile-test output directory is available");

    for (fixture, expected) in [
        ("operation_on_variant.rs", "catalogs only assemble types"),
        ("operation_request_argument.rs", "the operation is its request type"),
        ("unsafe_id.rs", "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments"),
        ("zero_window.rs", "a stream window must retain at least 1 row"),
        ("absent_axis_field.rs", "the declared `axis` names no field of this row"),
        ("duplicate_id.rs", "__HARMOS_STREAM_ID_70756c7365"),
        ("unregistered_row.rs", "Stream<Studio>"),
        ("non_serde_row.rs", "Serialize"),
        // The four ways a declared inverse is refused, in the order an author
        // meets them: saying nothing, saying something the other change does
        // not say back, naming a change the catalog does not contain, and
        // describing a reversal for a change declared irreversible.
        ("unanswered_undo.rs", "a transaction declares what undoes it"),
        ("unpaired_inverse.rs", "the declared inverse of `Place` does not name `Place` back"),
        ("uncatalogued_inverse.rs", "the declared inverse `Remove` is not a member of `Change`"),
        ("irreversible_body.rs", "`SendReport` declares no inverse"),
        // The default lane is the one every job that named no lane lands on,
        // so a catalog answers where exactly once and that lane is unbounded.
        ("capacitated_default_lane.rs", "the default lane is unbounded"),
        ("defaultless_lanes.rs", "exactly one lane `#[default]`"),
        ("doubly_defaulted_lanes.rs", "exactly one lane `#[default]`"),
        ("operation_owned_request.rs", "request and endpoints must be borrowed"),
        // Direct artifact declaration failures.
        ("explicit_version_plugin.rs", "remove `version`"),
        ("plugin_on_struct.rs", "belongs on `impl Plugin for ConcreteType`"),
        ("synchronous_activation.rs", "initializing awaits"),
        ("synchronous_deactivation.rs", "stopping awaits"),
        ("empty_catalog.rs", "observers = Observers"),
        ("undeclared_observer_output.rs", "observer outputs must be a subset"),
        ("undeclared_custom_observer_output.rs", "observer outputs must be a subset"),
        // A requirement grants its own operation, scoped by name; an empty
        // or unknown scope declares nothing a planner could read.
        ("empty_grant.rs", "grant scopes are nonempty"),
        ("unknown_grant.rs", "a requirement scopes its operation's grant"),
        ("malformed_harmos_path.rs", "`harmos_path` names the crate path"),
        // A message field owns a nested message directly or through one
        // `Box<T>`; every other boxed or shared shape is refused.
        ("boxed_scalar_field.rs", "unsupported boxed message field"),
        ("boxed_container_field.rs", "unsupported boxed message field"),
        ("nested_box_field.rs", "unsupported boxed message field"),
        ("optional_box_field.rs", "unsupported boxed message field"),
        ("repeated_box_field.rs", "unsupported boxed message field"),
        ("shared_message_field.rs", "unsupported boxed message field"),
    ] {
        let source = fixtures.join(fixture);
        let compiled = fixtures::command(&rustc)
            .arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("--emit=metadata")
            .arg(&source)
            .arg("--extern")
            .arg(format!("harmos={}", harmos.display()))
            .arg("--extern")
            .arg(format!("harmos_sidecar={}", harmos_sidecar.display()))
            .arg("--extern")
            .arg(format!("serde={}", serde.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--out-dir")
            .arg(&output)
            .output()
            .expect("rustc runs the compile-fail fixture");
        let stderr = String::from_utf8_lossy(&compiled.stderr);

        assert!(!compiled.status.success(), "{fixture} unexpectedly compiled successfully");
        assert!(stderr.contains(expected), "{fixture} did not report `{expected}`:\n{stderr}");
    }
}
