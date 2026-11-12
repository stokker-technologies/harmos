/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Locate the Stamped Guest Components

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
// Third Party Imports
use serde_json::Value;
use sha2::{Digest, Sha256};
// Local Imports

/* === Public Functions === */

pub fn component(name: &str) -> Vec<u8> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/build")
        .join(std::env::var("STKR_TEST_PROFILE").unwrap_or_else(|_| "dev".into()))
        .join(format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
        .join("fixtures");
    let focused = std::env::var_os("HARMOS_FOCUSED_FIXTURES").map(PathBuf::from);
    let manifest = focused.clone().unwrap_or_else(|| directory.join("stamp.json"));
    let stamp: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    let path = PathBuf::from(stamp["metadata"]["artifacts"][name].as_str().expect("the fixture stamp names the component"));
    let path = if path.is_absolute() { path } else { directory.join(path) };
    let bytes = std::fs::read(&path).unwrap();
    if focused.is_some() {
        let expected = stamp["metadata"]["inspection_sha256"][name].as_str().expect("the focused stamp has a digest");
        assert_eq!(Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>(), expected);
    }
    bytes
}

/* === Tests === */
