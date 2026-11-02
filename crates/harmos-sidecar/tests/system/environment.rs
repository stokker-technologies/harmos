/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Preserve Windows Paths While Filtering Ambient Process Variables.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

// Third Party Imports
use tokio::io::AsyncReadExt;
use tokio::process::Command;

// Local Imports
use harmos_sidecar::{Attach, Executable};

/* === Constants === */

const SCENE: &str = "HARMOS_ENVIRONMENT_SCENE";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn windows_environment_passes() {
    tokio::time::timeout(Duration::from_secs(120), async {
        if let Some(directory) = std::env::var_os(SCENE) {
            let directory = PathBuf::from(directory);
            let bytes = std::fs::read(directory.join("expected.json")).unwrap();
            let mut expected: BTreeMap<String, String> = serde_json::from_slice(&bytes).unwrap();
            // Windows May Normalize Program Files Aliases When Starting This Parent Process.
            for (name, value) in &mut expected {
                *value = std::env::var(name).unwrap();
            }
            let fixture = crate::prepared_fixtures::binary(
                "harmos-sidecar-catalog-fixture",
                option_env!("CARGO_BIN_EXE_harmos-sidecar-catalog-fixture"),
            );
            let mut executable = Executable::at(fixture)
                .unwrap()
                .argument("--environment-probe")
                .directory(directory.join("child"))
                .variable("HARMOS_EXPLICIT", "kept");
            if !expected.contains_key("APPDATA") {
                let profile = directory.join("chosen-profile").into_os_string();
                expected.insert("USERPROFILE".into(), profile.to_str().unwrap().into());
                executable = executable.variable("USERPROFILE", profile);
            }
            let attached = executable.attach().await.unwrap();
            let (_, mut read, _, diagnostics, ending) = attached.into_parts();
            let mut diagnostics = diagnostics.unwrap();
            let mut output = Vec::new();
            let mut errors = String::new();
            let (read_result, error_result) = tokio::join!(read.read_to_end(&mut output), diagnostics.read_to_string(&mut errors));
            read_result.unwrap();
            error_result.unwrap();
            ending.unwrap().settle().await.unwrap();
            let actual: BTreeMap<String, String> = serde_json::from_slice(&output).expect(&errors);
            for (name, value) in &expected {
                let received = actual.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value);
                assert_eq!(received, Some(value), "child variable {name}");
            }
            assert_eq!(actual.get("HARMOS_EXPLICIT").map(String::as_str), Some("kept"));
            for name in ["PATH", "HARMOS_AMBIENT_SECRET", SCENE] {
                assert!(!actual.keys().any(|key| key.eq_ignore_ascii_case(name)), "ambient variable {name} leaked");
            }
            if !expected.contains_key("APPDATA") {
                for name in ["APPDATA", "USERDOMAIN"] {
                    assert!(!actual.keys().any(|key| key.eq_ignore_ascii_case(name)), "absent variable {name} was invented");
                }
                let root = actual.iter().find(|(key, _)| key.eq_ignore_ascii_case("SystemRoot")).unwrap().1;
                assert!(std::path::Path::new(root).is_absolute());
                assert!(std::path::Path::new(root).is_dir());
            }
            assert!(directory.join("ProgramData/harmos-probe").is_dir());
            assert!(!directory.join("child/%SystemDrive%").exists());
            assert_eq!(std::fs::read_dir(directory.join("child")).unwrap().count(), 0);
            return;
        }

        let temporary = tempfile::tempdir().unwrap();
        for missing in [false, true] {
            let directory = temporary.path().join(if missing { "overridden" } else { "inherited" });
            std::fs::create_dir_all(directory.join("child")).unwrap();
            let mut expected: BTreeMap<String, String> = [
                "SystemDrive",
                "SystemRoot",
                "windir",
                "ProgramData",
                "ALLUSERSPROFILE",
                "ProgramFiles",
                "ProgramFiles(x86)",
                "ProgramW6432",
                "CommonProgramFiles",
                "CommonProgramFiles(x86)",
                "CommonProgramW6432",
                "PUBLIC",
                "USERPROFILE",
                "HOMEDRIVE",
                "HOMEPATH",
                "LOCALAPPDATA",
                "APPDATA",
                "USERNAME",
                "USERDOMAIN",
                "COMPUTERNAME",
                "PATHEXT",
                "ComSpec",
                "NUMBER_OF_PROCESSORS",
                "PROCESSOR_ARCHITECTURE",
                "OS",
            ]
            .into_iter()
            .map(|name| (name.into(), format!("parent-{name}-é")))
            .collect();
            let program_data = directory.join("ProgramData");
            let path = program_data.to_str().unwrap();
            let drive = &path[..2];
            assert!(drive.ends_with(':'));
            expected.insert("SystemDrive".into(), drive.into());
            expected.insert("ProgramData".into(), format!("%SystemDrive%{}", &path[2..]));
            expected.insert("SystemRoot".into(), std::env::var("SystemRoot").unwrap());
            expected.insert("windir".into(), std::env::var("SystemRoot").unwrap());
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.creation_flags(0x0800_0000);
            if missing {
                for name in ["APPDATA", "USERDOMAIN", "SystemRoot"] {
                    expected.remove(name);
                    command.env_remove(name);
                }
            }
            let bytes = serde_json::to_vec(&expected).unwrap();
            std::fs::write(directory.join("expected.json"), bytes).unwrap();
            let output = command
                .args(["--exact", "environment::windows_environment_passes", "--nocapture"])
                .envs(&expected)
                .env(SCENE, &directory)
                .env("HARMOS_AMBIENT_SECRET", "must-not-leak")
                .env("PATH", "must-not-leak")
                .kill_on_drop(true)
                .output()
                .await
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "environment scene failed:\n{stdout}\n{stderr}");
            assert!(stdout.contains("1 passed; 0 failed"), "the selected child test must run:\n{stdout}\n{stderr}");
        }
    })
    .await
    .expect("the environment story must settle within its hang guard");
}
