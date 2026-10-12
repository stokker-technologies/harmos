/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

mod export;
#[path = "../../../../tests/support/fixtures.rs"]
mod prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{path::Path, process::Stdio, time::Duration};

// Third Party Imports
use tokio::process::Command;

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
fn contract_scenario() {
    {
        scenario_runtime::check("protobuf_export_bytes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = tempfile::tempdir().unwrap();
                    let artifact = root.path().join("plugin.data");
                    // A Non-Executable Copy Must Suffice; The Plugin's Lifecycle Panics If Entered.
                    std::fs::write(
                        &artifact,
                        std::fs::read(prepared_fixtures::artifact(
                            "harmos-sidecar-catalog-fixture",
                        ))
                        .unwrap(),
                    )
                    .unwrap();
                    let project = root.path().join("project");
                    std::fs::create_dir_all(project.join("src")).unwrap();
                    std::fs::create_dir(project.join(".cargo")).unwrap();
                    std::fs::write(project.join("src/lib.rs"), "").unwrap();
                    std::fs::write(
                        project.join("Cargo.toml"),
                        r#"[package]
name = "export-fixture"
version = "0.1.0"
edition = "2024"
[workspace]
"#,
                    )
                    .unwrap();
                    std::fs::write(
                        project.join("Cargo.lock"),
                        r#"version = 4
[[package]]
name = "export-fixture"
version = "0.1.0"
"#,
                    )
                    .unwrap();
                    std::fs::write(
                        project.join(".cargo/config.toml"),
                        r#"[build]
target-dir = "configured-target"
"#,
                    )
                    .unwrap();
                    for override_target in [
                        Some("CARGO_TARGET_DIR"),
                        Some("CARGO_BUILD_TARGET_DIR"),
                        None,
                    ] {
                        let target = match override_target {
                            Some(variable) => root.path().join(variable),
                            None => project.join("configured-target"),
                        };
                        let mut command =
                            Command::new(prepared_fixtures::artifact("harmos-sidecar-export"));
                        command
                            .arg(&artifact)
                            .arg(project.join("Cargo.toml"))
                            .arg("native")
                            .stdin(Stdio::null())
                            .kill_on_drop(true);
                        // Exercise Each Cargo Override Independently Of The Parent Build Runner.
                        command
                            .env_remove("CARGO_TARGET_DIR")
                            .env_remove("CARGO_BUILD_TARGET_DIR");
                        if let Some(variable) = override_target {
                            command.env(variable, &target);
                        }
                        let output =
                            tokio::time::timeout(Duration::from_secs(30), command.output())
                                .await
                                .unwrap()
                                .unwrap();
                        assert!(
                            output.status.success(),
                            "{}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        let exported = target.join("grpc/native");
                        let reported = String::from_utf8(output.stdout).unwrap();
                        assert_eq!(
                            Path::new(reported.trim()).canonicalize().unwrap(),
                            exported.canonicalize().unwrap()
                        );
                        assert!(exported.join("descriptors.bin").is_file());
                        let proto =
                            std::fs::read_to_string(exported.join("fixture/v1/Native.proto"))
                                .unwrap();
                        assert!(proto.contains("rpc Read ("), "{proto}");
                    }
                });
        });
    }
}
