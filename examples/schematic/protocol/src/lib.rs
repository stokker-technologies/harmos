/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Schematic Messages Shared by Native Handlers and SDK Clients.

#[cfg(all(feature = "sdk", not(target_arch = "wasm32")))]
#[rustfmt::skip]
pub mod client;

#[rustfmt::skip]
mod host {
    #[rustfmt::skip]
    pub mod v1 {
        include!("host.rs");
    }
}
mod model {
    #[rustfmt::skip]
    pub mod v1 {
        include!("model.rs");
    }
}
pub use host::v1::*;
pub use model::v1::*;
#[rustfmt::skip]
#[path = "reflection.rs"]
mod reflection;
#[rustfmt::skip]
pub mod plugin {
    mod wire {
        include!("plugin.rs");
    }
    pub use wire::*;
}
#[cfg(feature = "sdk")]
use harmos_protocol::model::{SettingDeclaration, SettingValue, SettingsDeclarations, setting_value::Value};
use prost_reflect::DescriptorPool;
use std::sync::OnceLock;

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| DescriptorPool::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated schematic descriptors"))
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for plugin::SchematicValidateRequest {
    type Response = plugin::SchematicValidateResponse;
    const ID: &'static str = "schematic.plugin.v1.Schematic/Validate";
}
#[cfg(feature = "sdk")]
impl harmos::OperationRequest for plugin::SchematicProposeRouteRequest {
    type Response = plugin::SchematicProposeRouteResponse;
    const ID: &'static str = "schematic.plugin.v1.Schematic/ProposeRoute";
}
#[cfg(feature = "sdk")]
impl harmos::OperationRequest for plugin::SchematicListDirectoriesRequest {
    type Response = plugin::SchematicListDirectoriesResponse;
    const ID: &'static str = "schematic.plugin.v1.Schematic/ListDirectories";
}

/// Declare the Library-Stored Routing Preference Shared by Both Plugins.
#[cfg(feature = "sdk")]
pub fn settings_declarations() -> SettingsDeclarations {
    SettingsDeclarations {
        values: vec![SettingDeclaration {
            name: "horizontal_first".into(),
            title: "Horizontal First".into(),
            description: "Route horizontally before moving vertically.".into(),
            default: Some(SettingValue { value: Some(Value::Flag(true)) }),
        }],
    }
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for FilesHashRequest {
    type Response = FilesHashResponse;
    const ID: &'static str = "schematic.host.v1.Files/Hash";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for FilesReadRequest {
    type Response = FilesReadResponse;
    const ID: &'static str = "schematic.host.v1.Files/Read";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsCreateRequest {
    type Response = UploadsCreateResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Create";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsGetRequest {
    type Response = UploadsGetResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Get";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsResumeRequest {
    type Response = UploadsResumeResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Resume";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsAppendRequest {
    type Response = UploadsAppendResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Append";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsPublishRequest {
    type Response = UploadsPublishResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Publish";
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for UploadsDeleteRequest {
    type Response = UploadsDeleteResponse;
    const ID: &'static str = "schematic.host.v1.Uploads/Delete";
}
