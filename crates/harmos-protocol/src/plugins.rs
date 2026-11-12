/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Manager Contract Identity

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{collections::BTreeMap, sync::LazyLock};
// Third Party Imports
use prost::Message;
use prost_reflect::ReflectMessage;
// Local Imports
use crate::{host, schema};

/* === Constants === */

static CONTRACTS: LazyLock<BTreeMap<String, [u8; 32]>> = LazyLock::new(|| {
    let descriptor = host::PluginsGetRequest::default().descriptor();
    let types = prost_types::FileDescriptorSet {
        file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let mut contracts = BTreeMap::new();
    for method in ["Get", "List", "Watch", "Create", "Set", "Delete", "Restart"] {
        let request = format!("harmos.host.v1.Plugins{method}Request");
        let response = format!("harmos.host.v1.Plugins{method}Response");
        let descriptors = schema::operation_types(&types, &[&request, &response]).expect("generated plugin manager descriptors");
        let id = format!("harmos.plugins.{}", method.to_ascii_lowercase());
        let protobuf = host::Protobuf {
            descriptors: descriptors.encode_to_vec(),
            request,
            response: response.clone(),
            input: None,
            output: (method == "Watch").then_some(response),
        };
        let fingerprint = schema::host_fingerprint(&id, &protobuf).expect("generated plugin manager fingerprint");
        contracts.insert(id, fingerprint.try_into().expect("SHA-256 fingerprint"));
    }
    contracts
});

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Identify Only the Seven Exact Protocol-Owned Management Contracts.
pub fn fingerprint(id: &str) -> Option<[u8; 32]> {
    CONTRACTS.get(id).copied()
}

/* === Private Functions === */

/* === Tests === */
