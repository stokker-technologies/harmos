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

#[path = "../../../../tests/support/fixtures.rs"]
mod fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{fs, path::PathBuf, process::Command};
// Third Party Imports

// Local Imports

/* === Constants === */
const OWNED: [&str; 7] = [
    "plugin",
    "contracts",
    "contributions",
    "publications",
    "listeners",
    "observers",
    "restart",
];

const UNDEBUGGABLE: &str = "plugin-fixture";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn build(version: &str, guest: bool, profile: &str) -> PathBuf {
    fixtures::artifact(&format!("metadata:{version}:{guest}:{profile}"))
}

fn artifact(version: &str, guest: bool, profile: &str) -> Vec<u8> {
    fs::read(build(version, guest, profile)).expect("preparation produces the artifact")
}

fn fragments(bytes: &[u8]) -> impl Iterator<Item = serde_json::Value> + '_ {
    let prefix = b"\0HARMOS:ARTIFACT:V6:";
    bytes
        .windows(prefix.len())
        .enumerate()
        .filter_map(move |(index, value)| {
            if value != prefix {
                return None;
            }
            let rest = &bytes[index + prefix.len()..];
            let end = rest.iter().position(|byte| *byte == 0)?;
            serde_json::from_slice(&rest[..end]).ok()
        })
}

fn identity(bytes: &[u8], kind: &str) -> serde_json::Value {
    fragments(bytes)
        .find(|value| value["kind"] == kind)
        .expect("the executable carries its complete identity")
}

fn schema(bytes: &[u8], name: &serde_json::Value) -> serde_json::Value {
    assert!(name.is_string());
    fragments(bytes)
        .find(|value| value["kind"] == "schema" && value["name"] == *name)
        .expect("every referenced definition has retained inert metadata")
}

fn operation_inventory(bytes: &[u8], facility: &str, owner: &str) {
    let inventory = identity(bytes, "contracts");
    assert_eq!(inventory["facility"], facility);
    assert_eq!(inventory["owner"], owner);
    let provides = inventory["provides"]
        .as_array()
        .expect("provided wire contracts");
    let expected_ids: &[&str] = if facility == "sidecar" {
        &["selected.read", "fixture.start", "fixture.stop"]
    } else {
        &["selected.read"]
    };
    assert_eq!(
        provides
            .iter()
            .map(|contract| contract["operation"]["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        expected_ids
    );
    assert_eq!(inventory["requires"], serde_json::json!([]));
    assert_eq!(
        provides[0]["operation"],
        serde_json::json!({
            "id": "selected.read",
            "description": "Selected \"reading\" μ",
            "policy": {
                "timeout": {"secs": 2, "nanos": 0},
                "attempt_timeout": {"secs": 0, "nanos": 17},
                "retries": {"on_transient": {
                    "max_retries": 3,
                    "backoff": {"fixed": {
                        "delay": {"secs": 0, "nanos": 7},
                        "jitter": "deterministic"
                    }}
                }}
            },
            "visibility": "shared",
            "replay": "idempotent"
        })
    );
    assert_eq!(provides[0]["version"], 3);
    assert_eq!(provides[0]["codec"], "postcard");
    let request = schema(bytes, &provides[0]["request"]);
    let response = schema(bytes, &provides[0]["response"]);
    assert_eq!(request["fields"], response["fields"]);
    let fields = request["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["name"], "value");
    assert!(fields[0]["tag"].as_u64().unwrap() > 0);
    assert_eq!(fields[0]["kind"], serde_json::json!({ "type": "u32" }));
    assert_eq!(fields[0]["cardinality"], "required");
    let sdk = inventory["lifecycle"].as_array().unwrap();
    assert_eq!(sdk.len(), 2);
    for (contract, action) in sdk.iter().zip(["initialize", "shutdown"]) {
        assert_eq!(contract["action"], action);
        assert_eq!(contract["version"], 1);
        assert_eq!(
            contract["policy"],
            serde_json::json!({
                "timeout": {"secs": 2, "nanos": 0}, "attempt_timeout": {"secs": 1, "nanos": 0}, "retries": "never"
            })
        );
    }
    let contributions = identity(bytes, "contributions");
    assert_eq!(contributions["facility"], facility);
    assert_eq!(contributions["owner"], owner);
    let sections = if facility == "sidecar" {
        serde_json::json!([{"namespace": "fixture.catalog", "payload": "{\"kinds\":[\"probe\"]}"}])
    } else {
        serde_json::json!([])
    };
    assert_eq!(contributions["sections"], sections);
}

fn retention(version: &str) {
    let executable = build(version, false, UNDEBUGGABLE);
    let embedded = fs::read(&executable).expect("Cargo produces the artifact");
    operation_inventory(&embedded, "sidecar", "cargo-native");
    let scanned = OWNED.map(|kind| identity(&embedded, kind));
    let inventory = identity(&embedded, "observers");
    let observers = inventory["observers"].as_array().unwrap();
    assert_eq!(observers.len(), 1);
    assert_eq!(observers[0]["id"], "selected.watch");
    assert_eq!(observers[0]["streams"], serde_json::json!(["reading"]));
    assert_eq!(observers[0]["interval"]["nanos"], 17_000_000);
    let contracts = identity(&embedded, "contracts");
    assert_eq!(
        inventory["observer_controls"],
        serde_json::json!([contracts["provides"][1], contracts["provides"][2]])
    );
    assert_eq!(
        observers[0]["demand_keys"],
        serde_json::json!(["fixture:record"])
    );

    let reached = Command::new(&executable)
        .arg("--markers")
        .output()
        .expect("the built sidecar runs");
    assert!(
        reached.status.success(),
        "{}",
        String::from_utf8_lossy(&reached.stderr)
    );

    for declaration in fragments(&reached.stdout).filter(|value| value["kind"] == "schema") {
        assert_eq!(schema(&embedded, &declaration["name"]), declaration);
    }
    for (kind, scanned) in OWNED.iter().zip(scanned) {
        assert_eq!(
            identity(&reached.stdout, kind),
            scanned,
            "live code does not reach the embedded {kind} fragment, so a linker \
             that sweeps unreferenced statics would drop it"
        );
    }
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("cargo_refreshes_native", || {
            // The description is explicit declaration metadata, so it is fixed in
            // the fixture's source and its awkward characters exercise the serializer
            // the macro escapes it with rather than Cargo's.
            let description = "Quoted \"probe\" line \\ μ\u{1}";
            for version in ["7.8.9", "7.8.10-beta.2+build"] {
                for (guest, kind, id) in [
                    (false, "sidecar", "cargo-native"),
                    (true, "guest", "cargo-guest"),
                ] {
                    let bytes = artifact(version, guest, "debug");
                    let declaration = identity(&bytes, "plugin");
                    assert_eq!(declaration["facility"], kind);
                    operation_inventory(&bytes, kind, id);
                    assert_eq!(declaration["id"], id);
                    assert_eq!(declaration["version"], version);
                    assert_eq!(declaration["description"], description);
                    if version == "7.8.10-beta.2+build" {
                        let stripped = artifact(version, guest, "stripped-fixture");
                        assert_eq!(
                            identity(&stripped, "plugin"),
                            declaration,
                            "stripped development metadata differs for {kind}"
                        );
                        operation_inventory(&stripped, kind, id);
                        let publications = identity(&stripped, "publications");
                        assert_eq!(publications["facility"], kind);
                        assert_eq!(publications["owner"], id);
                        let row = schema(&stripped, &publications["streams"][0]["row"]);
                        assert_eq!(
                            row["fields"],
                            serde_json::json!([
                                {"name": "value", "tag": 39772037, "kind": {"type": "u32"}, "cardinality": "required"}
                            ])
                        );
                        assert_eq!(
                            publications["streams"],
                            serde_json::json!([{
                                "id": "reading", "schema": 1, "window": 7,
                                "metadata": {"identifier": "reading", "label": "Reading", "description": "A portable reading", "tags": [], "attributes": {}},
                                "row_metadata": [{
                                    "metadata": {"identifier": "value", "label": "Measured Value", "description": null, "tags": ["electrical"], "attributes": {"channel": {"type": "unsigned", "value": "1"}}},
                                    "role": "value", "unit": "V"
                                }],
                                "row": row["name"]
                            }])
                        );
                        let operations = identity(&stripped, "contracts");
                        assert_eq!(
                            operations["owner"], id,
                            "stripped development lost the owned inventory"
                        );
                    }
                    if version == "7.8.10-beta.2+build" && !guest {
                        retention(version);
                    }
                }
            }
        });
    }
}
