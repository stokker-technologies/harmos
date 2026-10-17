/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../../harmos/tests/support/fixtures.rs"]
mod prepared_fixtures;
mod source_scenarios;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
// Third Party Imports
use tokio::io::AsyncReadExt;
use tokio::process::Command;
// Local Imports
use harmos_sidecar::{Attach, Executable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn isolated_networking() {
    check(None).await;
}

#[cfg(windows)]
#[tokio::test]
async fn missing_system_root() {
    check(Some(None)).await;
    check(Some(Some(""))).await;
}

#[tokio::test]
async fn ambient_temporary_directory() {
    let ambient = fresh_directory("ambient-host");
    std::fs::create_dir_all(&ambient).unwrap();
    check_temp_directory(&ambient, None).await;
    std::fs::remove_dir_all(&ambient).unwrap();
}

#[tokio::test]
async fn explicit_temporary_directory() {
    let ambient = fresh_directory("explicit-host");
    let overridden = fresh_directory("explicit-override");
    std::fs::create_dir_all(&ambient).unwrap();
    std::fs::create_dir_all(&overridden).unwrap();
    check_temp_directory(&ambient, Some(&overridden)).await;
    std::fs::remove_dir_all(&ambient).unwrap();
    std::fs::remove_dir_all(&overridden).unwrap();
}

#[tokio::test]
async fn attach_controller() {
    if std::env::var_os("HARMOS_ATTACH_CONTROLLER").is_none() {
        return;
    }
    assert!(std::env::var_os("HARMOS_ATTACH_HOST_ONLY").is_some());
    let mut executable = Executable::at(prepared_fixtures::binary("harmos-sidecar-attach-fixture"))
        .unwrap()
        .variable("HARMOS_ATTACH_EXPLICIT", "configured");
    if let Some(expected) = std::env::var_os("HARMOS_ATTACH_EXPECT_TEMP_DIR") {
        executable = executable.variable("HARMOS_ATTACH_EXPECT_TEMP_DIR", expected);
    }
    if let Some(overridden) = std::env::var_os("HARMOS_ATTACH_EXPLICIT_TEMP_DIR") {
        executable = executable.variable(host_temp_variable(), overridden);
    }
    assert_eq!(executable.operation_contracts().unwrap(), Some(Vec::new()));
    let attached = executable.attach().await.unwrap();
    let (write, mut read, _control, diagnostics, end) = attached.into_parts();
    drop(write);
    let mut diagnostics = diagnostics.unwrap();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(
            read.read_to_string(&mut stdout),
            diagnostics.read_to_string(&mut stderr),
        )
    })
    .await;
    // Reap the attached child even if its networking check hangs or fails.
    let mut ending = end.unwrap();
    ending.settle().await.unwrap();
    result
        .expect("attached child must finish within 10 seconds")
        .unwrap();
    assert_eq!(stdout, "networking-ok\n", "child diagnostics: {stderr}");
}

fn host_temp_variable() -> &'static str {
    if cfg!(windows) { "TMP" } else { "TMPDIR" }
}

fn fresh_directory(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("harmos-attach-{label}-{}", std::process::id()))
}

async fn check_temp_directory(ambient: &Path, explicit_override: Option<&Path>) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "attach_controller", "--nocapture"])
        .env("HARMOS_ATTACH_CONTROLLER", "1")
        .env("HARMOS_ATTACH_HOST_ONLY", "must-not-leak")
        .env(host_temp_variable(), ambient)
        .env(
            "HARMOS_ATTACH_EXPECT_TEMP_DIR",
            explicit_override.unwrap_or(ambient),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(directory) = explicit_override {
        command.env("HARMOS_ATTACH_EXPLICIT_TEMP_DIR", directory);
    }
    let output = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .expect("attachment controller must finish within 30 seconds")
        .expect("attachment controller starts");
    assert!(
        output.status.success(),
        "attachment controller failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn check(system_root: Option<Option<&str>>) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "attach_controller", "--nocapture"])
        .env("HARMOS_ATTACH_CONTROLLER", "1")
        .env("HARMOS_ATTACH_HOST_ONLY", "must-not-leak")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match system_root {
        Some(Some(value)) => {
            command.env("SystemRoot", value);
        }
        Some(None) => {
            command.env_remove("SystemRoot");
        }
        None => {}
    }
    let output = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .expect("attachment controller must finish within 30 seconds")
        .expect("attachment controller starts");
    assert!(
        output.status.success(),
        "attachment controller failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/* === Tests === */
