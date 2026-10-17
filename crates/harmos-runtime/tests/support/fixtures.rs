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
use std::path::PathBuf;
use std::sync::OnceLock;

// Third Party Imports
use serde_json::Value;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn artifact(name: &str) -> PathBuf {
    let path = stamp()["metadata"]["artifacts"][name]
        .as_str()
        .unwrap_or_else(|| panic!("the fixture stamp has no {name}"));
    directory().join(path)
}

pub fn component(name: &str) -> Vec<u8> {
    std::fs::read(artifact(name))
        .unwrap_or_else(|error| panic!("stamped component {name} is unreadable: {error}"))
}

pub fn input(name: &str) -> PathBuf {
    directory().join(stamp()["metadata"][name].as_str().unwrap_or_else(|| panic!("fixture stamp has no {name} input")))
}

pub fn library(name: &str) -> PathBuf {
    directory().join(
        stamp()["metadata"]["libraries"][name]
            .as_str()
            .unwrap_or_else(|| panic!("the fixture stamp has no {name} library")),
    )
}

pub fn dependencies() -> PathBuf {
    directory().join(
        stamp()["metadata"]["dependencies"]
            .as_str()
            .expect("the fixture stamp names the dependency directory"),
    )
}

pub fn binary(name: &str) -> PathBuf {
    let variable = format!("NEXTEST_BIN_EXE_{}", name.replace('-', "_"));
    std::env::var_os(&variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{variable} is unset; run the story under nextest"))
}

/* === Private Functions === */
fn directory() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("nextest sets the manifest directory");
    manifest
        .ancestors()
        .nth(2)
        .expect("crates live two levels below the workspace")
        .join("target/fixtures")
}

fn stamp() -> &'static Value {
    static STAMP: OnceLock<Value> = OnceLock::new();
    STAMP.get_or_init(|| {
        let path = directory().join("stamp.json");
        let bytes = std::fs::read(&path).unwrap_or_else(|error| {
            panic!("fixture stamp {} is unreadable: {error}", path.display())
        });
        serde_json::from_slice(&bytes).expect("the fixture stamp is JSON")
    })
}

/* === Tests === */
