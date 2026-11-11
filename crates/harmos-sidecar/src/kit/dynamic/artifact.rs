/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Export The Existing Inert Artifact Dictionary From A Frozen Runtime Catalog.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde_json::{Value, json};

// Local Imports
use super::{DynamicPlugin, RuntimePlugin};
use harmos_domain::model::operations::lifecycle::Policy;
use harmos_domain::operations::{Contract, WireDeclaration};
use harmos_domain::schema::graph::{Kind, Schema};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn encode(plugin: &DynamicPlugin) -> std::io::Result<Vec<u8>> {
    let mut fragments = vec![json!({
        "kind": "plugin", "facility": "sidecar", "id": plugin.id, "version": "0.1.0",
        "description": "Native SDK prototype", "config": "null",
    })];
    fragments.push(json!({
        "kind": "contracts", "facility": "sidecar", "owner": plugin.id,
        "entry": {"direction": "lifecycle", "lifecycle": [
            {"action": "initialize", "identity": "harmos.operation.v1", "policy": RuntimePlugin::INITIALIZATION_POLICY},
            {"action": "shutdown", "identity": "harmos.operation.v1", "policy": RuntimePlugin::SHUTDOWN_POLICY},
        ]},
    }));
    let provides = plugin.operations.iter().map(|operation| &operation.contract);
    for (index, contract) in provides.enumerate() {
        let prefix = format!("native.operation.{index}");
        let named = named_contract(contract, &prefix, &mut fragments);
        fragments.push(json!({"kind": "contracts", "facility": "sidecar", "owner": plugin.id,
            "entry": {"direction": "provides", "contract": named}}));
    }
    if !plugin.streams.is_empty() {
        let publish = WireDeclaration::of::<harmos_operations::streams::Publish>().contract().map_err(std::io::Error::other)?;
        let named = named_contract(&publish, "native.publish", &mut fragments);
        fragments.push(json!({"kind": "contracts", "facility": "sidecar", "owner": plugin.id,
            "entry": {"direction": "requires", "contract": named, "grants": []}}));
    }
    let mut streams = Vec::new();
    for (index, stream) in plugin.streams.iter().enumerate() {
        let prefix = format!("native.stream.{index}");
        dictionary(&stream.row, &prefix, &mut fragments);
        streams.push(json!({"id": stream.id, "window": stream.window,
            "metadata": stream.metadata, "row_metadata": stream.row_metadata, "row": format!("{prefix}.{}", stream.row.root)}));
    }
    fragments.push(json!({"kind": "publications", "facility": "sidecar", "owner": plugin.id, "streams": streams}));
    fragments.push(json!({"kind": "observers", "facility": "sidecar", "owner": plugin.id, "observers": []}));
    let mut bytes = Vec::new();
    for fragment in fragments {
        // Avoid Embedding A Complete Search Marker In The Exporter's Own Binary.
        let family = std::hint::black_box(b"\0HARMOS:ARTIFACT:\0");
        bytes.extend_from_slice(&family[..family.len() - 1]);
        bytes.extend_from_slice(std::hint::black_box(b"V1:"));
        serde_json::to_writer(&mut bytes, &fragment)?;
        bytes.push(0);
    }
    let declaration = harmos_domain::inventory::Declaration::inspect(&bytes, harmos_domain::inventory::Facility::Sidecar)
        .map_err(std::io::Error::other)?;
    let expected: Vec<_> = plugin.operations.iter().map(|operation| operation.contract.operation()).collect();
    let actual: Vec<_> = declaration.contracts.provides.iter().map(Contract::operation).collect();
    if actual != expected || declaration.streams != plugin.streams {
        return Err(std::io::Error::other("inert catalog differs from runtime declarations"));
    }
    Ok(bytes)
}

/* === Private Functions === */

fn named_contract(contract: &Contract, prefix: &str, fragments: &mut Vec<Value>) -> Value {
    let schema = contract.schema();
    dictionary(schema, prefix, fragments);
    let operation = contract.operation();
    json!({
        "operation": {"id": operation.id, "description": operation.description, "policy": operation.policy,
            "visibility": operation.visibility, "replay": operation.replay},
        "identity": "harmos.operation.v1", "payload_codec": contract.codec().id(), "response_codec": contract.codec().id(),
        "input_codec": null, "output_codec": null, "codec": contract.codec(),
        "request": format!("{prefix}.{}", contract.request().index()),
        "response": format!("{prefix}.{}", contract.response().index()),
    })
}

fn dictionary(schema: &Schema, prefix: &str, fragments: &mut Vec<Value>) {
    for (index, fields) in schema.definitions.iter().enumerate() {
        let fields: Vec<_> = fields
            .iter()
            .map(|field| {
                let kind = match field.kind {
                    Kind::Message { definition } => json!({"type": "message", "definition": format!("{prefix}.{definition}")}),
                    kind => serde_json::to_value(kind).expect("scalar kind is serializable"),
                };
                json!({"name": field.name, "tag": field.tag, "kind": kind, "cardinality": field.cardinality})
            })
            .collect();
        fragments.push(json!({"kind": "schema", "name": format!("{prefix}.{index}"), "fields": fields}));
    }
}

/* === Tests === */
