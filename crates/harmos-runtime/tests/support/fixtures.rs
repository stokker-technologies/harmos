/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Locate The Fixtures The One Build Stamped And The Package Binaries Nextest Exposes
#![allow(dead_code)]

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

// Third Party Imports
use serde_json::Value;
use sha2::{Digest, Sha256};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x08000000);
        command
    };
    command
}

pub fn manifest() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from).expect("nextest sets the manifest directory")
}

pub fn workspace() -> PathBuf {
    manifest().ancestors().nth(2).expect("crates live two levels below the workspace").to_owned()
}

pub fn artifact(name: &str) -> PathBuf {
    let path = stamp()["metadata"]["artifacts"][name].as_str().unwrap_or_else(|| panic!("the fixture stamp has no {name}"));
    directory().join(path)
}

// Tests Own These Inert Copies; Inspecting Them Never Loads Or Executes A Plugin.
pub fn inspection(name: &str) -> Vec<u8> {
    let path = stamp()["metadata"]["inspections"][name].as_str().unwrap_or_else(|| panic!("the fixture stamp has no {name} inspection"));
    let path = directory().join(path);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("inspection fixture {name} is unreadable: {error}"));
    if focused_manifest() {
        let expected = stamp()["metadata"]["inspection_sha256"][name]
            .as_str()
            .unwrap_or_else(|| panic!("the focused fixture manifest has no digest for {name} inspection"));
        verify_digest(&bytes, expected, &format!("focused inspection {name}"));
    }
    bytes
}

pub fn input(name: &str) -> PathBuf {
    let input = stamp()["metadata"][name].as_str().unwrap_or_else(|| panic!("fixture stamp has no {name} input"));
    directory().join(input)
}

pub fn library(name: &str) -> PathBuf {
    let value = &stamp()["metadata"]["libraries"][name];
    let fallback = value["path"].as_str();
    let path = value.as_str().or(fallback);
    let path = path.unwrap_or_else(|| panic!("the fixture stamp has no {name} library"));
    let path = resolve(path);
    if focused_manifest() {
        let expected = value["sha256"].as_str().unwrap_or_else(|| panic!("the focused fixture manifest has no digest for {name} library"));
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("focused library {name} is unreadable: {error}"));
        verify_digest(&bytes, expected, &format!("focused library {name}"));
    }
    path
}

pub fn dependencies() -> PathBuf {
    let value = &stamp()["metadata"]["dependencies"];
    let fallback = value["path"].as_str();
    let path = value.as_str().or(fallback);
    let path = path.expect("the fixture stamp names the dependency directory");
    resolve(path)
}

// Nextest Names The Extracted Executable At Run Time; Plain `cargo test` Only Has The Compile-Time Path.
pub fn binary(name: &str, compiled: Option<&str>) -> PathBuf {
    let variable = format!("NEXTEST_BIN_EXE_{}", name.replace('-', "_"));
    let path = std::env::var_os(&variable).map(PathBuf::from);
    let path = match path {
        Some(path) => Some(path),
        None => compiled.map(PathBuf::from),
    };
    path.unwrap_or_else(|| panic!("{variable} is unset and the package does not own {name}"))
}

// The Vendored Protobuf Compiler Travels With The Stamped Offline Inputs, Not The Build Runner's Registry
pub fn protoc() -> PathBuf {
    let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch_64",
        ("macos", "x86_64") => "macos-x86_64",
        ("macos", "aarch64") => "macos-aarch_64",
        ("windows", _) => "win32",
        other => panic!("no vendored protoc for {other:?}"),
    };

    let prefix = format!("protoc-bin-vendored-{platform}-");
    let vendor = input("vendor");
    let mut entries = std::fs::read_dir(&vendor).unwrap();

    let package = entries.find_map(|entry| {
        let path = entry.unwrap().path();
        vendor_package(&path, &prefix).then_some(path)
    });
    let package = package.unwrap_or_else(|| panic!("the stamped vendor directory has no {prefix}* package"));
    let executable = if cfg!(windows) { "protoc.exe" } else { "protoc" };
    package.join("bin").join(executable)
}

/* === Private Functions === */

fn directory() -> PathBuf {
    if let Some(path) = std::env::var_os("HARMOS_FOCUSED_FIXTURES") {
        let path = PathBuf::from(path);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("focused fixture manifest {} is unreadable: {error}", path.display()));
        let value: Value = serde_json::from_slice(&bytes).expect("focused fixture manifest is JSON");
        return PathBuf::from(value["metadata"]["directory"].as_str().expect("focused manifest names its dependency directory"));
    }
    let profile = match std::env::var("STKR_TEST_PROFILE") {
        Ok(profile) if matches!(profile.as_str(), "dev" | "release") => profile,
        Err(std::env::VarError::NotPresent) => "dev".to_owned(),
        _ => panic!("STKR_TEST_PROFILE must be dev or release"),
    };
    let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    workspace().join("target/build").join(profile).join(target).join("fixtures")
}

fn focused_manifest() -> bool {
    std::env::var_os("HARMOS_FOCUSED_FIXTURES").is_some()
}

fn verify_digest(bytes: &[u8], expected: &str, label: &str) {
    let digest = Sha256::digest(bytes);
    let actual = digest.iter().map(hex_byte).collect::<String>();
    assert_eq!(actual, expected, "{label} digest differs from the focused fixture manifest");
}

fn hex_byte(byte: &u8) -> String {
    format!("{byte:02x}")
}

fn vendor_package(path: &std::path::Path, prefix: &str) -> bool {
    let Some(file_name) = path.file_name() else { return false };
    let Some(name) = file_name.to_str() else { return false };
    name.starts_with(prefix)
}

fn stamp() -> &'static Value {
    static STAMP: OnceLock<Value> = OnceLock::new();
    STAMP.get_or_init(|| {
        let path = match std::env::var_os("HARMOS_FOCUSED_FIXTURES") {
            Some(path) => PathBuf::from(path),
            None => directory().join("stamp.json"),
        };
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("fixture stamp {} is unreadable: {error}", path.display()));
        let value: Value = serde_json::from_slice(&bytes).expect("the fixture stamp is JSON");
        if std::env::var_os("HARMOS_FOCUSED_FIXTURES").is_some() {
            assert_eq!(value["schema"].as_u64(), Some(1), "focused fixture manifest schema is supported");
        }
        value
    })
}

fn resolve(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() { path } else { directory().join(path) }
}

/* === Tests === */
