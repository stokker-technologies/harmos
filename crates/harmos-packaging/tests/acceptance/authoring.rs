/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Compile And Test Each Generated Starter Against Staged Offline Inputs.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// Third Party Imports

// Local Imports
use super::fixtures;


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Project {
    directory: tempfile::TempDir,
    root: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Project {
    fn new(kind: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join(format!("starter-{kind}"));
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let result = Command::new(fixtures::artifact("cargo-harmos")).current_dir(checkout)
            .args(["harmos", "new", kind]).arg(&root).arg("--harmos-path").arg(checkout).output().unwrap();
        succeeded(&result);
        let project = Self { directory, root };
        let home = project.directory.path().join("cargo-home");
        std::fs::create_dir(&home).unwrap();
        let vendor = serde_json::to_string(&fixtures::input("vendor")).unwrap();
        std::fs::write(home.join("config.toml"), format!("[net]\noffline = true\n[source.crates-io]\nreplace-with = \"vendored\"\n[source.vendored]\ndirectory = {vendor}\n")).unwrap();
        project.run("git", &["init", "--initial-branch=feat/starter"]);
        project.run("mise", &["trust", "mise.toml"]);
        project.run("cargo", &["generate-lockfile", "--offline"]);
        project.run("uv", &["sync", "--locked", "--offline", "--all-groups", "--project", "tools/python"]);
        project
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.root)
            .env("CARGO_HOME", self.directory.path().join("cargo-home"))
            .env("CARGO_TARGET_DIR", self.root.join("target"))
            .env("CARGO_NET_OFFLINE", "true")
            .env("UV_PROJECT_ENVIRONMENT", self.root.join(".venv"))
            .env("UV_CACHE_DIR", self.directory.path().join("uv-cache"))
            .env("UV_OFFLINE", "1").env("UV_NO_INDEX", "1")
            .env("UV_FIND_LINKS", fixtures::input("wheels"));
        command
    }

    fn run(&self, program: &str, arguments: &[&str]) {
        let output = self.command(program).args(arguments).output().unwrap();
        succeeded(&output);
    }

    fn check_and_test(&self) {
        self.run("mise", &["run", "check"]);
        self.run("mise", &["run", "build"]);
        self.run("mise", &["run", "test:unit"]);
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.command("mise").args(["untrust", "mise.toml"]).output();
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn succeeded(output: &Output) {
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

/* === Tests === */

#[test]
fn sidecar_starter_checks_and_tests() {
    Project::new("sidecar").check_and_test();
}

#[test]
fn guest_starter_checks_and_tests() {
    Project::new("guest").check_and_test();
}
