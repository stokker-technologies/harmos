/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeSet;
// Third Party Imports
use serde_json::{Value, json};

// Local Imports

/* === Constants === */
const PREFIX: &[u8] = b"\0HARMOS:ARTIFACT:V6:";

const OWNED: [&str; 5] = ["contracts", "contributions", "publications", "listeners", "observers"];

const SHARED: [&str; 3] = ["schematic.analyze", "schematic.route", "schematic.validate"];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn fragments(bytes: &[u8]) -> impl Iterator<Item = Value> + '_ {
    bytes.windows(PREFIX.len()).enumerate().filter_map(move |(index, value)| {
        if value != PREFIX {
            return None;
        }
        let rest = &bytes[index + PREFIX.len()..];
        let end = rest.iter().position(|byte| *byte == 0)?;
        serde_json::from_slice(&rest[..end]).ok()
    })
}

fn identity(bytes: &[u8], kind: &str) -> Value {
    fragments(bytes).find(|value| value["kind"] == kind).unwrap_or_else(|| panic!("the artifact carries its {kind} fragment"))
}

fn schema(bytes: &[u8], name: &Value) -> Value {
    assert!(name.is_string());
    fragments(bytes)
        .find(|value| value["kind"] == "schema" && value["name"] == *name)
        .unwrap_or_else(|| panic!("referenced definition {name} has retained inert metadata"))
}

fn inventory(bytes: &[u8], facility: &str, expected: &BTreeSet<&str>) {
    // Owned Fragments
    for kind in OWNED {
        let fragment = identity(bytes, kind);
        assert_eq!(fragment["facility"], facility, "{facility} {kind} facility");
        assert_eq!(fragment["owner"], "schematic-tools", "{facility} {kind} owner");
    }

    // Provided Contracts
    let contracts = identity(bytes, "contracts");
    let provides = contracts["provides"].as_array().expect("provided wire contracts");
    let ids = provides.iter().map(|contract| contract["operation"]["id"].as_str().expect("operation id")).collect::<BTreeSet<_>>();
    assert_eq!(&ids, expected, "{facility} provided operations");
    assert_eq!(contracts["requires"], json!([]));
    for contract in provides {
        schema(bytes, &contract["request"]);
        schema(bytes, &contract["response"]);
    }

    // Lifecycle Contracts
    let lifecycle = contracts["lifecycle"].as_array().expect("lifecycle contracts");
    assert_eq!(lifecycle.len(), 2);
    for (contract, action) in lifecycle.iter().zip(["initialize", "shutdown"]) {
        assert_eq!(contract["action"], action);
        assert_eq!(contract["version"], 1);
        assert_eq!(
            contract["policy"],
            json!({
                "timeout": {"secs": 10, "nanos": 0},
                "attempt_timeout": {"secs": 10, "nanos": 0},
                "retries": "never",
                "restart": "never"
            })
        );
    }

    // Undeclared Sections
    assert_eq!(identity(bytes, "contributions")["sections"], json!([]));
    assert_eq!(identity(bytes, "publications")["streams"], json!([]));
    assert_eq!(identity(bytes, "observers")["observers"], json!([]));
}

/* === Tests === */

#[test]
fn artifacts_embed_metadata() {
    let native = SHARED.into_iter().chain(["schematic.analyze_exchange", "schematic.analyze_upload", "schematic.list_directories"]).collect::<BTreeSet<_>>();
    let portable = SHARED.into_iter().collect::<BTreeSet<_>>();

    // Read The Built Artifacts
    let sidecar = fixtures::component("schematic-sidecar");
    let guest = fixtures::component("schematic-guest");

    // Compare Both Declarations
    let mut declarations = Vec::new();
    for (bytes, facility, expected) in [(&sidecar, "sidecar", &native), (&guest, "guest", &portable)] {
        let declaration = identity(bytes, "plugin");
        assert_eq!(declaration["facility"], facility);
        assert_eq!(declaration["id"], "schematic-tools");
        assert!(declaration["version"].as_str().is_some_and(|version| !version.is_empty()), "{facility} declares the package version");
        inventory(bytes, facility, expected);
        declarations.push([declaration["id"].clone(), declaration["version"].clone(), declaration["description"].clone()]);
    }
    assert_eq!(declarations[0], declarations[1], "one plugin built for both facilities declares one id, version and description");
}
