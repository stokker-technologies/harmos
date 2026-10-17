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
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::process::{Child, Command, ExitStatus, Output, Stdio};

// Third Party Imports
use serde_json::Value;

// Local Imports
use super::fixtures;


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Watching {
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    events: mpsc::Receiver<Value>,
    log: PathBuf,
}

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
        let wheels = serde_json::to_string(&fixtures::input("wheels")).unwrap();
        std::fs::write(project.directory.path().join("uv.toml"), format!("no-index = true\nfind-links = [{wheels}]\n")).unwrap();
        project.run("git", &["init", "--initial-branch=feat/starter"]);
        project.run("mise", &["trust", "mise.toml"]);
        project.run("cargo", &["generate-lockfile", "--offline"]);
        project.run("uv", &["sync", "--locked", "--offline", "--all-groups", "--project", "tools/python"]);
        project.run("git", &["add", "."]);
        project.run("git", &["-c", "user.name=Harmos Fixture", "-c", "user.email=fixture@harmos.invalid", "commit", "-m", "feat: create authored plugin"]);
        project
    }

    fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.root)
            .env("CARGO_HOME", self.directory.path().join("cargo-home"))
            .env("CARGO_TARGET_DIR", self.root.join("target"))
            .env("CARGO_NET_OFFLINE", "true")
            .env("UV_PROJECT_ENVIRONMENT", self.root.join(".venv"))
            .env("UV_CACHE_DIR", self.directory.path().join("uv-cache"))
            .env("UV_OFFLINE", "1")
            .env("UV_CONFIG_FILE", self.directory.path().join("uv.toml"));
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

impl Watching {
    fn start(project: &Project, kind: &str) -> Self {
        let log = project.directory.path().join("watch.log");
        let mut command = project.command(fixtures::artifact("cargo-harmos"));
        command.args(["harmos", kind, "dev", "--json", "--debounce-ms", "1000"])
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(std::fs::File::create(&log).unwrap());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
        }
        let mut child = command.spawn().unwrap();
        let output = child.stdout.take().unwrap();
        let (sender, events) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let value: Value = serde_json::from_str(&line.unwrap()).unwrap();
                if sender.send(value).is_err() { break; }
            }
        });
        Self { child: Some(child), reader: Some(reader), events, log }
    }

    fn event(&self, expected: &str) -> Value {
        let value = self.events.recv().unwrap_or_else(|_| panic!("missing {expected}: {}", std::fs::read_to_string(&self.log).unwrap()));
        assert_eq!(value["event"].as_str(), Some(expected), "{value}");
        value
    }

    fn built(&self, attempt: u64) -> (PathBuf, String) {
        let started = self.event("started");
        assert_eq!(started["attempt"], attempt);
        let built = self.event("built");
        assert_eq!(built["attempt"], attempt);
        let artifacts = built["build"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 1);
        (PathBuf::from(artifacts[0]["path"].as_str().unwrap()), artifacts[0]["sha256"].as_str().unwrap().into())
    }

    fn stop(&mut self) -> ExitStatus {
        let mut child = self.child.take().unwrap();
        assert!(child.try_wait().unwrap().is_none(), "watching remains active before cancellation");
        interrupt(child.id());
        let status = child.wait().unwrap();
        self.reader.take().unwrap().join().unwrap();
        status
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if child.try_wait().ok().flatten().is_none() { interrupt(child.id()); }
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() { let _ = reader.join(); }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.command("mise").args(["untrust", "mise.toml"]).output();
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(unix)]
fn interrupt(pid: u32) {
    let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status();
}

#[cfg(windows)]
fn interrupt(pid: u32) {
    let code = format!("Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class Signal {{ [DllImport(\"kernel32.dll\", SetLastError=true)] public static extern bool GenerateConsoleCtrlEvent(uint kind, uint group); }}'; if (-not [Signal]::GenerateConsoleCtrlEvent(1, {pid})) {{ exit 1 }}");
    let _ = Command::new("powershell").args(["-NoProfile", "-Command", &code]).status();
}

fn digest(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(std::fs::read(path).unwrap()).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn dev_rebuilds(kind: &str) {
    let project = Project::new(kind);
    let dependency = project.directory.path().join("dependency");
    std::fs::create_dir_all(dependency.join("src")).unwrap();
    std::fs::write(dependency.join("Cargo.toml"), "[package]\nname = \"watch-dependency\"\nversion = \"0.0.0\"\nedition = \"2024\"\n").unwrap();
    let dependent = dependency.join("src/lib.rs");
    std::fs::write(&dependent, "pub fn value() -> u32 { 10 }\n").unwrap();
    let manifest = project.root.join("Cargo.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("\n[dependencies.watch-dependency]\npath = \"../dependency\"\n");
    std::fs::write(&manifest, text).unwrap();
    project.run("cargo", &["generate-lockfile", "--offline"]);
    let source = project.root.join("src/domain/model/greeting/mod.rs");
    let original = std::fs::read_to_string(&source).unwrap();
    assert!(original.contains("format!(\"Hello, {name}!\")"), "the starter greeting remains the authored build input");
    let authored = original.replace("format!(\"Hello, {name}!\")", "format!(\"Hello, {name}! {}\", WATCH_SOURCE + watch_dependency::value())") + "\nconst WATCH_SOURCE: u32 = 1;\n";
    std::fs::write(&source, &authored).unwrap();
    let mut watcher = Watching::start(&project, kind);
    let (artifact, initial) = watcher.built(1);
    assert_eq!(digest(&artifact), initial);

    // Source And Local Dependency Changes Reach Real Compiled Artifact Bytes.
    for _ in 0..10 {
        std::fs::write(&source, authored.replace("WATCH_SOURCE: u32 = 1", "WATCH_SOURCE: u32 = 2")).unwrap();
    }
    let (same, after_source) = watcher.built(2);
    assert_eq!(same, artifact);
    assert_ne!(after_source, initial);
    std::fs::write(&dependent, "pub fn value() -> u32 { 20 }\n").unwrap();
    let (_, after_dependency) = watcher.built(3);
    assert_ne!(after_dependency, after_source);

    // Failed Builds Retain The Last Usable Artifact Until Recovery.
    std::fs::write(&dependent, "compile_error!(\"watch failure\");\n").unwrap();
    assert_eq!(watcher.event("started")["attempt"], 4);
    assert_eq!(watcher.event("failed")["attempt"], 4);
    assert_eq!(digest(&artifact), after_dependency);
    std::fs::write(&dependent, "pub fn value() -> u32 { 30 }\n").unwrap();
    let (_, recovered) = watcher.built(5);
    assert_ne!(recovered, after_dependency);

    // A Build Script Owns A Python Descendant Until Product Cancellation Reaps Both.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let python = project.root.join(".venv").join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" });
    let program = format!("import json,os,socket; s=socket.create_connection(('127.0.0.1',{})); s.sendall(json.dumps([os.getppid(),os.getpid()]).encode()+b'\\n'); s.recv(1)", address.port());
    let script = format!("fn main() {{ std::process::Command::new({:?}).args([\"-c\", {:?}]).status().unwrap(); }}\n", python, program);
    std::fs::write(project.root.join("build.rs"), script).unwrap();
    assert_eq!(watcher.event("started")["attempt"], 6);
    let (mut descendant, _) = listener.accept().unwrap();
    let mut line = String::new();
    BufReader::new(descendant.try_clone().unwrap()).read_line(&mut line).unwrap();
    let owned: Vec<u32> = serde_json::from_str(&line).unwrap();
    assert_eq!(owned.len(), 2);
    assert!(owned.iter().all(|pid| *pid != 0));
    assert!(watcher.stop().success(), "cancellation is an orderly tool outcome");
    let mut trailing = Vec::new();
    descendant.read_to_end(&mut trailing).unwrap();
    assert!(trailing.is_empty(), "EOF witnesses the owned descendant's exit");
}

fn succeeded(output: &Output) {
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

/* === Tests === */

#[test]
fn sidecar_starter_works() {
    Project::new("sidecar").check_and_test();
}

#[test]
fn guest_starter_works() {
    Project::new("guest").check_and_test();
}

#[test]
fn sidecar_dev_lifecycle() {
    dev_rebuilds("sidecar");
}

#[test]
fn guest_dev_lifecycle() {
    dev_rebuilds("guest");
}
