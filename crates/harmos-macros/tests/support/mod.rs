/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/fixtures.rs"]
pub mod fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::path::PathBuf;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn libraries() -> BTreeMap<String, PathBuf> {
    fixtures::manifest()["libraries"]
        .as_object()
        .expect("prepared compiler libraries")
        .iter()
        .map(|(name, value)| {
            let path = fixtures::resolve(value.as_str().expect("prepared library path"));
            assert!(
                path.is_file(),
                "prepared library {} is missing; run mise run test:integration:rust",
                path.display()
            );
            (name.clone(), path)
        })
        .collect()
}

pub fn dependencies(kind: &str) -> PathBuf {
    let path = fixtures::resolve(
        fixtures::manifest()["dependencies"][kind]
            .as_str()
            .expect("prepared dependency directory"),
    );
    assert!(
        path.is_dir(),
        "prepared dependencies {} are missing; run mise run test:integration:rust",
        path.display()
    );
    path
}

/* === Private Functions === */

/* === Tests === */
