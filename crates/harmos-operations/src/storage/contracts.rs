/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Validate Reserved Storage Declarations Against Their Descriptor Owner

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::{Contract, ContractError, Grpc, OperationDefinition, Protobuf, ProtobufExt, Visibility};
use harmos_domain::config::SchemaSettings;
use prost_reflect::DescriptorPool;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn contract(
    operation: &OperationDefinition, protobuf: &Protobuf, binding: Option<&Grpc>, settings: &SchemaSettings,
) -> Result<Option<Contract>, ContractError> {
    let id = operation.id.as_str();
    let package = if id.starts_with("harmos.v1.Storage/") {
        "harmos.v1"
    } else if id.starts_with("harmos.context.v1.Storage/") {
        "harmos.context.v1"
    } else {
        return Ok(None);
    };
    let refused = || ContractError::Protobuf(format!("storage declaration {id} differs from its authoritative face contract"));
    static POOL: std::sync::OnceLock<DescriptorPool> = std::sync::OnceLock::new();
    let pool = POOL.get_or_init(|| {
        DescriptorPool::from_file_descriptor_set(harmos_protocol::storage_types().clone()).expect("generated storage descriptors")
    });
    let service = pool.get_service_by_name(&format!("{package}.Storage")).ok_or_else(refused)?;
    let method = service.methods().find(|method| operation_id(package, method.name()) == id).ok_or_else(refused)?;
    let expected_binding = (package == "harmos.v1").then(|| Grpc {
        package: package.to_owned().into(),
        service: "Storage".into(),
        managed: false,
        method: method.name().to_owned().into(),
    });
    let fingerprint =
        harmos_protocol::schema::fingerprint(id, harmos_protocol::storage_types(), method.input().full_name(), method.output().full_name())
            .map_err(|_| refused())?;
    if protobuf.request != method.input().full_name()
        || protobuf.response != method.output().full_name()
        || protobuf.input.is_some()
        || protobuf.output.is_some()
        || binding != expected_binding.as_ref()
        || operation.visibility != Visibility::Public
        || protobuf.fingerprint(id)?.0.as_slice() != fingerprint.as_slice()
    {
        return Err(refused());
    }
    operation.policy.validate().map_err(crate::operations::Error::from)?;
    Contract::generated(operation.clone(), protobuf.clone(), binding.cloned(), settings).map(Some)
}

/* === Private Functions === */

fn operation_id(package: &str, method: &str) -> String {
    format!("{package}.Storage/{method}")
}

/* === Tests === */
