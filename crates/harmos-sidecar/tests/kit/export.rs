/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

use super::prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::process::Stdio;
use std::time::Duration;
// Third Party Imports
use tokio::process::Command;
// Local Imports
use harmos_sidecar::{CATALOG_EXPORT_FLAG, Registrar, Registration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn export(mode: &str) -> std::process::Output {
    let mut command = Command::new(prepared_fixtures::binary("harmos-sidecar-catalog-fixture"));
    command
        .arg(CATALOG_EXPORT_FLAG)
        .env("HARMOS_CATALOG_FIXTURE", mode)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .expect("export terminates without serving")
        .expect("fixture executes")
}

#[tokio::test]
async fn deterministic_source_catalog() {
    let first = export("valid").await;
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    let mut source = Registrar::new();
    source
        .register("zeta.catalog", 2, &serde_json::json!({"suite": "probe"}))
        .unwrap();
    source.register("alpha.catalog", 1, &vec![1, 2]).unwrap();
    let source = source.finish();
    assert_eq!(
        serde_json::from_slice::<Vec<Registration>>(&first.stdout).unwrap(),
        source
    );
    assert_eq!(first.stdout, serde_json::to_vec(&source).unwrap());
    assert_eq!(first.stdout, export("valid").await.stdout);
}

#[tokio::test]
async fn failure_omits_catalog() {
    for (mode, diagnostic) in [
        ("invalid", "declared more than once"),
        ("oversized", "exceeds the byte limit"),
    ] {
        let output = export(mode).await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(diagnostic), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
}

/* === Tests === */
