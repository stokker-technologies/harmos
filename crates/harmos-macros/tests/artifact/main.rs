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

const PREFIX: &[u8] = b"\0HARMOS:ARTIFACT:V1:";

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

fn inventory(bytes: &[u8], facility: &str, expected: &BTreeSet<&str>, required: &BTreeSet<&str>, published: &BTreeSet<&str>) {
    for kind in OWNED {
        let fragment = identity(bytes, kind);
        assert_eq!(fragment["facility"], facility, "{facility} {kind} facility");
        assert_eq!(fragment["owner"], "schematic_tools", "{facility} {kind} owner");
    }

    let contracts = fragments(bytes).filter(|value| value["kind"] == "contracts").collect::<Vec<_>>();
    let provides = contracts
        .iter()
        .filter(|value| value["entry"]["direction"] == "provides")
        .map(|value| &value["entry"]["contract"])
        .collect::<Vec<_>>();
    let ids = provides.iter().map(|contract| contract["operation"]["id"].as_str().expect("operation id")).collect::<BTreeSet<_>>();
    assert_eq!(&ids, expected, "{facility} provided operations");
    let requires = contracts
        .iter()
        .filter(|value| value["entry"]["direction"] == "requires")
        .map(|value| &value["entry"]["contract"])
        .collect::<Vec<_>>();
    let needed = requires.iter().map(|contract| contract["operation"]["id"].as_str().expect("operation id")).collect::<BTreeSet<_>>();
    assert_eq!(&needed, required, "{facility} required operations");
    for contract in provides.iter().chain(requires.iter()) {
        schema(bytes, &contract["request"]);
        schema(bytes, &contract["response"]);
    }

    let lifecycle = contracts.iter().find(|value| value["entry"]["direction"] == "lifecycle").unwrap()["entry"]["lifecycle"]
        .as_array()
        .expect("lifecycle contracts");
    assert_eq!(lifecycle.len(), 2);
    for (contract, action) in lifecycle.iter().zip(["initialize", "shutdown"]) {
        assert_eq!(contract["action"], action);
        assert_eq!(contract["identity"], "harmos.operation.v1");
        assert_eq!(
            contract["policy"],
            json!({
                "timeout": {"secs": 10, "nanos": 0},
                "attempt_timeout": {"secs": 10, "nanos": 0},
                "cancellation_timeout": {"secs": 0, "nanos": 0},
                "retries": "never"
            })
        );
    }

    assert_eq!(identity(bytes, "contributions")["sections"], json!([]));
    let streams = identity(bytes, "publications")["streams"].as_array().expect("published streams").clone();
    let streams = streams.iter().map(|stream| stream["id"].as_str().expect("stream id").to_owned()).collect::<BTreeSet<_>>();
    assert_eq!(streams, published.iter().map(|id| (*id).to_owned()).collect(), "{facility} published streams");
    assert_eq!(identity(bytes, "observers")["observers"], json!([]));
}

/* === Tests === */

#[test]
fn artifacts_embed_metadata() {
    let native = SHARED
        .into_iter()
        .chain(["schematic.analyze.exchange", "schematic.analyze.upload", "schematic.list.directories"])
        .collect::<BTreeSet<_>>();
    let publishing = BTreeSet::from(["host.publish"]);
    let progress = BTreeSet::from(["schematic.analysis_progress"]);
    let sidecar = fixtures::inspection("schematic-sidecar");
    let declaration = identity(&sidecar, "plugin");
    assert_eq!(declaration["facility"], "sidecar");
    assert_eq!(declaration["id"], "schematic_tools");
    assert!(declaration["version"].as_str().is_some_and(|version| !version.is_empty()), "sidecar declares the package version");
    inventory(&sidecar, "sidecar", &native, &publishing, &progress);
}
