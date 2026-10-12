/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::{Path, PathBuf};
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
pub fn path() -> PathBuf {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    std::env::var_os("HARMOS_FIXTURES_MANIFEST")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository.join("target/fixtures/harmos/consumer.json"))
}

pub fn manifest() -> &'static Value {
    static MANIFEST: OnceLock<Value> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let path = path();
        let bytes = std::fs::read(&path).unwrap_or_else(|error| {
            panic!("prepared fixture manifest {} is unavailable: {error}; run mise run test:integration:rust", path.display())
        });
        serde_json::from_slice(&bytes).expect("prepared fixture manifest is valid JSON")
    })
}

pub fn artifact(name: &str) -> PathBuf {
    let path = manifest()["artifacts"][name].as_str().unwrap_or_else(|| {
        panic!("prepared artifact {name} is missing; run mise run test:integration:rust")
    });
    let path = resolve(path);
    assert!(
        path.is_file(),
        "prepared artifact {} is missing; run mise run test:integration:rust",
        path.display()
    );
    path
}

pub fn resolve(relative: &str) -> PathBuf {
    let base = path();
    base.parent()
        .expect("fixture manifest parent")
        .join(relative)
}

pub fn component(example: &str, features: &str, case: Option<&str>) -> Vec<u8> {
    let key = format!("{example}:{features}:{}", case.unwrap_or("default"));
    let path = artifact(&key);
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read prepared artifact {}: {error}; run mise run test:integration:rust",
            path.display()
        )
    })
}

/* === Private Functions === */

/* === Tests === */
