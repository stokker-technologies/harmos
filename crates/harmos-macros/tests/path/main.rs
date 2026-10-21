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
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
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

fn author(rustc: &OsStr, fixtures: &Path, dependencies: &Path, output: &Path, facade: &Path, fixture: &str) -> Output {
    Command::new(rustc)
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg("--emit=metadata")
        .arg(fixtures.join(fixture))
        .arg("--extern")
        .arg(format!("facade={}", facade.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--out-dir")
        .arg(output)
        .output()
        .unwrap_or_else(|error| panic!("rustc runs {fixture}: {error}"))
}

/* === Tests === */

#[test]
fn crate_builds_against() {
    let harmos = fixtures::library("harmos");
    let serde = fixtures::library("serde");
    let dependencies = fixtures::dependencies();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = std::env::temp_dir().join(format!("harmos-facade-ui-{}", std::process::id()));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    fs::create_dir_all(&output).expect("the compile-test output directory is available");

    // Built once against Harmos and serde — never harmos-macros directly, since
    // the facade re-exports the attributes through harmos, exactly as
    // `crates/harmos` itself does.
    let compiled = Command::new(&rustc)
        .arg("--edition=2024")
        .arg("--crate-type=rlib")
        .arg("--crate-name=facade")
        .arg(fixtures.join("facade/lib.rs"))
        .arg("--extern")
        .arg(format!("harmos={}", harmos.display()))
        .arg("--extern")
        .arg(format!("serde={}", serde.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--out-dir")
        .arg(&output)
        .output()
        .expect("rustc builds the facade fixture");
    assert!(compiled.status.success(), "the facade fixture failed to build:\n{}", String::from_utf8_lossy(&compiled.stderr));
    let facade = output.join("libfacade.rlib");
    assert!(facade.is_file(), "the facade build did not produce {}", facade.display());

    for fixture in [
        "facade_message_with_override.rs",
        "facade_operations_with_override.rs",
        "facade_catalog_with_override.rs",
        "facade_lanes_with_override.rs",
    ] {
        let compiled = author(&rustc, &fixtures, &dependencies, &output, &facade, fixture);
        assert!(
            compiled.status.success(),
            "{fixture} failed to build against the facade alone:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }

    for fixture in [
        "facade_message_without_override.rs",
        "facade_operations_without_override.rs",
        "facade_catalog_without_override.rs",
        "facade_lanes_without_override.rs",
    ] {
        let compiled = author(&rustc, &fixtures, &dependencies, &output, &facade, fixture);
        let stderr = String::from_utf8_lossy(&compiled.stderr);
        assert!(
            !compiled.status.success(),
            "{fixture} unexpectedly built against the facade alone: the default `::harmos` path \
             must fail to resolve when `harmos` is not linked, or the override is not load-bearing"
        );
        assert!(stderr.contains("harmos"), "{fixture} failed for a reason other than the unresolved default path:\n{stderr}");
    }
}
