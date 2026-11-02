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
fn feature_catalog_compiles() {
    let sdk = fixtures::library("harmos_sdk");
    let dependencies = fixtures::dependencies();

    let source = fixtures::manifest().join("tests/fixtures/feature_catalog.rs");
    let directory = format!("harmos-cfg-catalog-{}", std::process::id());
    let output = std::env::temp_dir().join(directory);
    let rustc = std::env::var_os("RUSTC");
    let rustc = rustc.unwrap_or("rustc".into());
    fs::create_dir_all(&output).unwrap();

    let executable = output.join(if cfg!(windows) { "feature_catalog.exe" } else { "feature_catalog" });
    for features in [&[][..], &["--cfg", "feature=\"optional_operation\""][..]] {
        let mut compiler = fixtures::command(&rustc);
        compiler
            .args(["--edition=2024", "--crate-type=bin", "--crate-name=feature_catalog"])
            .arg(&source)
            .arg("--extern")
            .arg(format!("harmos={}", sdk.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--out-dir")
            .arg(&output)
            .args(features);

        let compiled = compiler.output().unwrap();
        let diagnostic = String::from_utf8_lossy(&compiled.stderr);
        assert!(compiled.status.success(), "catalog features {features:?} failed:\n{diagnostic}");

        let executed = fixtures::command(&executable).status().unwrap();
        assert!(executed.success(), "catalog features {features:?} inventory and registration failed");
    }
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn owner_ids_compile() {
    let harmos = fixtures::library("harmos");
    let dependencies = fixtures::dependencies();
    let output = std::env::temp_dir().join(format!("harmos-owner-ui-{}", std::process::id()));
    fs::create_dir_all(&output).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let cases: Vec<(String, bool)> =
        serde_json::from_str(include_str!("../../../harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
    for (index, (owner, valid)) in cases.into_iter().enumerate() {
        let source = output.join(format!("owner_{index}.rs"));
        let literal = format!("{owner:?}");
        let authored = format!(
            r#"
use harmos;
struct Example;
impl harmos::builtins::lifecycle::Policy for Example {{
    const INITIALIZATION_POLICY: harmos::OperationPolicy = harmos::OperationPolicy {{
        timeout: std::time::Duration::from_secs(10),
        attempt_timeout: std::time::Duration::from_secs(10),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: harmos::RetryPolicy::Never,
    }};
    const SHUTDOWN_POLICY: harmos::OperationPolicy = Self::INITIALIZATION_POLICY;
}}
impl harmos::Lifecycle for Example {{
    type Config = ();
    type Context = harmos::sidecar::Supervisor;
    type Error = std::convert::Infallible;
    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {{ Ok(Self) }}
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {{ Ok(()) }}
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {{ Ok(()) }}
    async fn cleanup(&mut self) -> Result<(), Self::Error> {{ Ok(()) }}
}}
#[harmos::sidecar::plugin(id = {literal}, requires = ())]
impl harmos::Plugin for Example {{
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}}
fn main() {{}}
"#
        );
        fs::write(&source, authored).unwrap();
        let compiled = fixtures::command(&rustc)
            .args(["--edition=2024", "--crate-type=bin", "--emit=metadata"])
            .env("CARGO_PKG_VERSION", env!("CARGO_PKG_VERSION"))
            .arg(&source)
            .arg("--extern")
            .arg(format!("harmos={}", harmos.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--out-dir")
            .arg(&output)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&compiled.stderr);
        assert_eq!(compiled.status.success(), valid, "plugin {owner:?}:\n{stderr}");
        if !valid {
            assert!(stderr.contains("plugin id must match"), "plugin {owner:?}:\n{stderr}");
        }
    }
    let source = fixtures::manifest().join("tests/fixtures/facade_runtime_catalogs.rs");
    let compiled = fixtures::command(&rustc)
        .args(["--edition=2024", "--crate-type=bin", "--crate-name=facade_runtime_catalogs"])
        .env("CARGO_PKG_VERSION", env!("CARGO_PKG_VERSION"))
        .arg(source)
        .arg("--extern")
        .arg(format!("harmos={}", harmos.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--out-dir")
        .arg(&output)
        .output()
        .unwrap();
    assert!(compiled.status.success(), "facade role catalogs failed to build:\n{}", String::from_utf8_lossy(&compiled.stderr));
    let executable = output.join(if cfg!(windows) { "facade_runtime_catalogs.exe" } else { "facade_runtime_catalogs" });
    let executed = fixtures::command(&executable).status().unwrap();
    assert!(executed.success(), "facade role inventory fixture failed at runtime");
    fs::remove_dir_all(output).unwrap();
}

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
        ("bad_operation_id.rs", "operation id"),
        ("undeclared_resource.rs", "Requires<u64>"),
        ("undeclared_operation.rs", "Invokes<Other>"),
        ("unsafe_id.rs", "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments"),
        ("zero_window.rs", "a stream window must retain at least 1 row"),
        ("absent_axis_field.rs", "the declared `axis` names no field of this row"),
        ("duplicate_id.rs", "__HARMOS_STREAM_ID_70756c7365"),
        ("unregistered_row.rs", "Stream<Studio>"),
        ("non_serde_row.rs", "Serialize"),
        ("unpaired_inverse.rs", "the declared inverse of `Place` does not name `Place` back"),
        ("uncatalogued_inverse.rs", "the declared inverse `Remove` is not a member of `Change`"),
        // The default lane is the one every job that named no lane lands on,
        // so a catalog answers where exactly once and that lane is unbounded.
        ("capacitated_default_lane.rs", "the default lane is unbounded"),
        ("defaultless_lanes.rs", "exactly one lane `#[default]`"),
        ("doubly_defaulted_lanes.rs", "exactly one lane `#[default]`"),
        ("operation_on_impl.rs", "operation belongs on the implementation struct"),
        // Direct artifact declaration failures.
        ("explicit_version_plugin.rs", "remove `version`"),
        ("plugin_on_struct.rs", "belongs on `impl Plugin for ConcreteType`"),
        ("plugin_setup_operations.rs", "ApplicationConfiguration"),
        ("synchronous_activation.rs", "initializing awaits"),
        ("synchronous_deactivation.rs", "stopping awaits"),
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

#[test]
fn grpc_visibility_compiles() {
    let sdk = fixtures::library("harmos_sdk");
    let dependencies = fixtures::dependencies();
    let output = tempfile::tempdir().unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    for (index, (visibility, accepted)) in [
        ("", false),
        ("visibility = Private,", false),
        ("visibility = harmos::operations::Visibility::Private,", false),
        ("visibility = PRIVATE,", false),
        ("visibility = Public,", true),
        ("visibility = Host,", true),
        ("visibility = harmos::operations::Visibility::Public,", true),
        ("visibility = PUBLIC,", true),
        ("visibility = Shared,", false),
        ("visibility = harmos::operations::Visibility::Shared,", false),
    ]
    .into_iter()
    .enumerate()
    {
        let source = output.path().join(format!("visibility_{index}.rs"));
        fs::write(
            &source,
            format!(
                r#"
#![allow(dead_code)]
const PRIVATE: harmos::operations::Visibility = harmos::operations::Visibility::Private;
const PUBLIC: harmos::operations::Visibility = harmos::operations::Visibility::Public;
#[harmos::operation(harmos_path = ::harmos, id = "visibility.call",
    policy = harmos::model::operations::lifecycle::POLICY, {visibility}
    grpc(package = "visibility.v1", service = "Operations", method = "Call"))]
struct Call;
fn main() {{}}
"#
            ),
        )
        .unwrap();
        let compiled = fixtures::command(&rustc)
            .args(["--edition=2024", "--crate-type=bin", "--emit=metadata"])
            .arg(&source)
            .arg("--extern")
            .arg(format!("harmos={}", sdk.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--out-dir")
            .arg(output.path())
            .output()
            .unwrap();
        let diagnostic = String::from_utf8_lossy(&compiled.stderr);
        assert_eq!(compiled.status.success(), accepted, "{visibility}:\n{diagnostic}");
        if !accepted {
            let expected = if visibility.contains("Shared") { "Shared" } else { "Private operations cannot declare a gRPC binding" };
            assert!(diagnostic.contains(expected), "{diagnostic}");
        }
    }
}
