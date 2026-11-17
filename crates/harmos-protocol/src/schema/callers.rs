/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Caller Options and Runtime Descriptor Projection.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, ExtensionDescriptor, MethodDescriptor, ReflectMessage, Value};
use prost_types::FileDescriptorSet;

// Local Imports
use super::{Error, Executor, operation_identity};
use crate::host::TimeGetRequest;
use crate::model::Callers;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Read The Mandatory Caller Option Without Dropping Unknown Descriptor Fields.
pub fn method_callers(method: &MethodDescriptor) -> Result<Callers, Error> {
    let id = format!("{}/{}", method.parent_service().full_name(), method.name());
    let extension = method
        .parent_pool()
        .get_extension_by_name("harmos.model.v1.callers")
        .ok_or_else(|| Error(format!("Declare {id}: Missing Callers Option Definition")))?;
    let options = method.options();
    if !options.has_extension(&extension) {
        return Err(Error(format!("Declare {id}: Missing Callers Option")));
    }
    let value = options.get_extension(&extension);
    let Value::EnumNumber(value) = value.as_ref() else {
        return Err(Error(format!("Declare {id}: Invalid Callers Option")));
    };
    match Callers::try_from(*value) {
        Ok(Callers::Clients) => Ok(Callers::Clients),
        Ok(Callers::Plugins) => Ok(Callers::Plugins),
        Ok(Callers::All) => Ok(Callers::All),
        _ => Err(Error(format!("Declare {id}: Invalid Callers Option"))),
    }
}

/// Attach Effective Caller Options To Runtime-Projected Service Descriptors.
pub fn with_method_callers(mut types: FileDescriptorSet, callers: &BTreeMap<String, Callers>) -> Result<DescriptorPool, Error> {
    let authority = TimeGetRequest::default().descriptor();
    let authority = authority.parent_pool();
    for name in ["google/protobuf/descriptor.proto", "harmos/model/v1/callers.proto"] {
        let declared = authority.get_file_by_name(name).expect("protocol caller option dependency");
        let declared = declared.file_descriptor_proto().clone();
        match types.file.iter_mut().find(|file| file.name.as_deref() == Some(name)) {
            Some(file) => *file = declared,
            None => types.file.push(declared),
        }
    }
    for file in &mut types.file {
        let host = file.package.as_deref().is_some_and(|package| package.split('.').any(|segment| segment == "host"));
        let projected = file.service.iter().any(|service| {
            service.method.iter().any(|method| {
                let id = format!(
                    "{}.{}/{}",
                    file.package.as_deref().unwrap_or_default(),
                    service.name.as_deref().unwrap_or_default(),
                    method.name.as_deref().unwrap_or_default()
                );
                callers.contains_key(&id)
            })
        });
        if (host || projected) && !file.service.is_empty() && !file.dependency.iter().any(|name| name == "harmos/model/v1/callers.proto") {
            file.dependency.push("harmos/model/v1/callers.proto".into());
        }
    }
    let meta = authority.get_message_by_name("google.protobuf.FileDescriptorSet").expect("descriptor set schema");
    let extension = authority.get_extension_by_name("harmos.model.v1.callers").expect("protocol caller option");
    let mut descriptors = DynamicMessage::decode(meta, types.encode_to_vec().as_slice()).map_err(|error| Error(error.to_string()))?;
    let files = descriptors.get_field_by_name_mut("file").and_then(Value::as_list_mut).expect("descriptor files");
    for file in files {
        let file = file.as_message_mut().expect("file descriptor");
        let package = file.get_field_by_name("package").expect("package field").as_str().unwrap_or_default().to_owned();
        let services = file.get_field_by_name_mut("service").and_then(Value::as_list_mut).expect("descriptor services");
        for service in services {
            let service = service.as_message_mut().expect("service descriptor");
            project_service(service, &package, callers, &extension)?;
        }
    }
    DescriptorPool::decode(descriptors.encode_to_vec().as_slice()).map_err(|error| Error(error.to_string()))
}

/* === Private Functions === */

// Dynamic Options Preserve Custom Fields That prost_types Does Not Model.
fn project_service(
    service: &mut DynamicMessage, package: &str, callers: &BTreeMap<String, Callers>, extension: &ExtensionDescriptor,
) -> Result<(), Error> {
    let service_name = service.get_field_by_name("name").expect("service name").as_str().unwrap_or_default().to_owned();
    let methods = service.get_field_by_name_mut("method").and_then(Value::as_list_mut).expect("descriptor methods");
    for method in methods {
        let method = method.as_message_mut().expect("method descriptor");
        let name = method.get_field_by_name("name").expect("method name").as_str().unwrap_or_default().to_owned();
        let id = format!("{package}.{service_name}/{name}");
        if operation_identity(&id)? == Executor::Plugin && !callers.contains_key(&id) {
            continue;
        }

        let caller = callers.get(&id).copied().ok_or_else(|| Error(format!("Declare {id}: Supply a Caller Projection")))?;
        if !matches!(caller, Callers::Clients | Callers::Plugins | Callers::All) {
            return Err(Error(format!("Declare {id}: Select Clients, Plugins or All for the Caller Projection")));
        }

        let options = method.get_field_by_name_mut("options").and_then(Value::as_message_mut).expect("method options");
        options.set_extension(extension, Value::EnumNumber(caller as i32));
    }
    Ok(())
}

/* === Tests === */
