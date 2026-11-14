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
    let sdk = fixtures::library("harmos_operations");
    let dependencies = fixtures::dependencies();

    let source = fixtures::manifest().join("tests/fixtures/feature_catalog.rs");
    let directory = format!("harmos-cfg-catalog-{}", std::process::id());
    let output = std::env::temp_dir().join(directory);
    let rustc = std::env::var_os("RUSTC");
    let rustc = rustc.unwrap_or("rustc".into());
    fs::create_dir_all(&output).unwrap();

    let executable = output.join(if cfg!(windows) {
        "feature_catalog.exe"
    } else {
        "feature_catalog"
    });
    for features in [&[][..], &["--cfg", "feature=\"optional_operation\""][..]] {
        let mut compiler = fixtures::command(&rustc);
        compiler
            .args(["--edition=2024", "--crate-type=bin", "--crate-name=feature_catalog"])
            .arg(&source)
            .arg("--extern")
            .arg(format!("harmos_operations={}", sdk.display()))
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
fn authoring_refusals_compile() {
    let harmos = fixtures::library("harmos_runtime");
    let serde = fixtures::library("serde");
    let dependencies = fixtures::dependencies();
    let fixtures = fixtures::manifest().join("tests/fixtures");
    let output = std::env::temp_dir().join(format!("harmos-stream-ui-{}", std::process::id()));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    fs::create_dir_all(&output).expect("the compile-test output directory is available");

    for (fixture, expected) in [
        ("serde_refusals.rs", "Serde attribute"),
        ("non_serde_row.rs", "Serialize"),
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
        if fixture == "serde_refusals.rs" {
            for (attribute, owner) in [
                ("flatten", "Flatten"),
                ("serialize_with", "Serializer"),
                ("with", "Module"),
                ("into", "Conversion"),
                ("remote", "Remote"),
                ("skip_serializing_if", "Predicate"),
                ("skip", "Positional"),
                ("transparent", "Transparent"),
                ("tag", "TaggedScalar"),
                ("untagged", "MixedVariant"),
            ] {
                let diagnostic = format!("Serde attribute `{attribute}` on type `{owner}`");
                assert!(stderr.contains(&diagnostic), "missing {diagnostic}:\n{stderr}");
            }
        }
    }
}
