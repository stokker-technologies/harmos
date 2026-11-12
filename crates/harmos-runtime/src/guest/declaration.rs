/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Protobuf Metadata

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::Error;
use harmos_operations::{
    inventory::contracts::Required,
    operations::{Contract, OperationDefinition, OperationPolicy, Protobuf, ReplaySafety, Visibility},
};
use harmos_protocol::v1;
use prost::Message as _;
use std::num::NonZeroUsize;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub id: String,
    pub version: semver::Version,
    pub operations: Vec<OperationDefinition>,
    pub contracts: Contracts,
    pub streams: Vec<harmos_operations::StreamDeclaration>,
    pub sections: Vec<v1::Section>,
    pub(crate) registration: v1::GuestRegistration,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contracts {
    pub provides: Vec<Contract>,
    pub requires: Vec<Required>,
    pub lifecycle: [harmos_operations::lifecycle::Descriptor; 2],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Declaration {
    pub(crate) fn new(registration: v1::GuestRegistration, stream_window: Option<NonZeroUsize>) -> Result<Self, Error> {
        let invalid = |message: String| Error::Guest { message };
        if !harmos_domain::identity::segment(&registration.id) {
            return Err(invalid("Invalid Guest Identity".into()));
        }
        let version = semver::Version::parse(&registration.version).map_err(|error| invalid(error.to_string()))?;
        let catalog = registration.catalog.as_ref().ok_or_else(|| invalid("Guest Catalog Is Required".into()))?;
        if catalog.digest != harmos_protocol::schema::catalog_digest(catalog) {
            return Err(invalid("Guest Catalog Digest Differs".into()));
        }
        if catalog.operations.len() > 4096 || catalog.requires.len() > 4096 || catalog.streams.len() > 4096 || catalog.sections.len() > 256
        {
            return Err(invalid("Guest Catalog Exceeds Its Bound".into()));
        }
        let types = catalog.types.as_ref().ok_or_else(|| invalid("Guest Descriptor Set Is Required".into()))?;
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types.clone()).map_err(|error| invalid(error.to_string()))?;
        let mut ids = std::collections::BTreeSet::new();
        let mut provides = Vec::new();
        for operation in &catalog.operations {
            if !ids.insert(&operation.id) {
                return Err(invalid("Duplicate Guest Operation".into()));
            }
            let contract = contract(operation, types)?;
            if contract.fingerprint().0.as_slice() != operation.fingerprint {
                return Err(invalid("Guest Operation Fingerprint Differs".into()));
            }
            provides.push(contract);
        }
        if registration.requirements.len() != catalog.requires.len() {
            return Err(invalid("Guest Requirements Differ".into()));
        }
        ids.clear();
        let mut requires = Vec::new();
        for required in &registration.requirements {
            if !ids.insert(&required.id)
                || !catalog.requires.iter().any(|entry| entry.id == required.id && entry.fingerprint == required.fingerprint)
            {
                return Err(invalid("Invalid or Duplicate Guest Requirement".into()));
            }
            let contract = contract(
                &v1::OperationContract {
                    id: required.id.clone(),
                    request: required.request.clone(),
                    response: required.response.clone(),
                    visibility: v1::Visibility::Public as i32,
                    ..Default::default()
                },
                types,
            )?;
            if contract.fingerprint().0.as_slice() != required.fingerprint {
                return Err(invalid("Guest Requirement Fingerprint Differs".into()));
            }
            if let Some(provided) = provides.iter().find(|provided| provided.operation().id == contract.operation().id) {
                if !provided.compatible(&contract) {
                    return Err(invalid("Self Requirement Differs from Its Provided Contract".into()));
                }
                continue;
            }
            requires.push(Required { contract, grants: Vec::new() });
        }
        ids.clear();
        let mut streams = Vec::new();
        for stream in &catalog.streams {
            if !ids.insert(&stream.id) || stream.interval.is_some() || !stream.interval_setting.is_empty() {
                return Err(invalid("Invalid or Unsupported Guest Stream".into()));
            }
            let window = stream_window.ok_or_else(|| invalid("The Host Must Supply a Guest Stream Window".into()))?;
            let descriptor = pool.get_message_by_name(&stream.row).ok_or_else(|| invalid("Guest Stream Row Is Missing".into()))?;
            let row = crate::streams::protobuf::Row::new(descriptor).map_err(|error| invalid(error.to_string()))?.schema;
            streams.push(harmos_operations::StreamDeclaration {
                metadata: harmos_domain::Metadata::new(
                    harmos_domain::Identifier::new(&stream.id).map_err(|error| invalid(error.to_string()))?,
                    &stream.id,
                    (!stream.description.is_empty()).then(|| stream.description.clone()),
                ),
                row_metadata: Vec::new(),
                id: stream.id.clone(),
                window: window.get(),
                row,
            });
        }
        ids.clear();
        for section in &catalog.sections {
            if !ids.insert(&section.r#type) {
                return Err(invalid("Duplicate Guest Section".into()));
            }
            let descriptor = pool.get_message_by_name(&section.r#type).ok_or_else(|| invalid("Guest Section Type Is Missing".into()))?;
            prost_reflect::DynamicMessage::decode(descriptor, section.payload.as_slice()).map_err(|error| invalid(error.to_string()))?;
        }
        let initialization = OperationPolicy::try_from(registration.initialization.as_ref()).map_err(|error| invalid(error.to_string()))?;
        let shutdown = OperationPolicy::try_from(registration.shutdown.as_ref()).map_err(|error| invalid(error.to_string()))?;
        let lifecycle = [
            harmos_operations::lifecycle::Descriptor::new(harmos_operations::lifecycle::Action::Initialize, initialization)
                .map_err(|error| invalid(error.to_string()))?,
            harmos_operations::lifecycle::Descriptor::new(harmos_operations::lifecycle::Action::Shutdown, shutdown)
                .map_err(|error| invalid(error.to_string()))?,
        ];
        Ok(Self {
            id: registration.id.clone(),
            version,
            operations: provides.iter().map(|contract| contract.operation().clone()).collect(),
            contracts: Contracts { provides, requires, lifecycle },
            streams,
            sections: catalog.sections.clone(),
            registration,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn contract(operation: &v1::OperationContract, types: &prost_types::FileDescriptorSet) -> Result<Contract, Error> {
    let invalid = |message: String| Error::Guest { message };
    let visibility = match operation.visibility() {
        v1::Visibility::Public => Visibility::Public,
        v1::Visibility::Private => Visibility::Private,
        _ => return Err(invalid("Invalid Guest Operation Visibility".into())),
    };
    let policy = OperationPolicy::try_from(operation.policy.as_ref()).map_err(|error| invalid(error.to_string()))?;
    let definition = OperationDefinition {
        id: harmos_domain::Identifier::new(&operation.id).map_err(|error| invalid(error.to_string()))?,
        description: operation.description.clone(),
        visibility,
        policy,
        replay: if operation.replay_safe { ReplaySafety::Idempotent } else { ReplaySafety::Unknown },
        fingerprint: harmos_domain::Fingerprint([0; 32]),
    };
    let protobuf = Protobuf {
        descriptors: harmos_protocol::schema::operation_types(types, &[&operation.request, &operation.response])
            .map_err(|error| invalid(error.to_string()))?
            .encode_to_vec(),
        request: operation.request.clone(),
        response: operation.response.clone(),
        input: None,
        output: None,
    };
    Contract::from_protobuf(definition, protobuf, None, &Default::default()).map_err(|error| invalid(error.to_string()))
}

/* === Tests === */
