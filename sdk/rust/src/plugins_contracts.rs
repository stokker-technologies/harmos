/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Management Client Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::{Message, OperationRequest};
use harmos_protocol::host as wire;
use prost_reflect::ReflectMessage;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for wire::PluginsGetRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsGetRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsGetResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsGetResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsGetRequest {
    type Response = wire::PluginsGetResponse;
    const ID: &'static str = "harmos.plugins.get";
}
impl Message for wire::PluginsListRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsListRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsListResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsListResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsListRequest {
    type Response = wire::PluginsListResponse;
    const ID: &'static str = "harmos.plugins.list";
}
impl Message for wire::PluginsWatchRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsWatchRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsWatchResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsWatchResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsCreateRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsCreateRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsCreateResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsCreateResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsCreateRequest {
    type Response = wire::PluginsCreateResponse;
    const ID: &'static str = "harmos.plugins.create";
}
impl Message for wire::PluginsSetRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsSetRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsSetResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsSetResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsSetRequest {
    type Response = wire::PluginsSetResponse;
    const ID: &'static str = "harmos.plugins.set";
}
impl Message for wire::PluginsDeleteRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsDeleteRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsDeleteResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsDeleteResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsDeleteRequest {
    type Response = wire::PluginsDeleteResponse;
    const ID: &'static str = "harmos.plugins.delete";
}
impl Message for wire::PluginsRestartRequest {
    fn name() -> String {
        "harmos.host.v1.PluginsRestartRequest".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl Message for wire::PluginsRestartResponse {
    fn name() -> String {
        "harmos.host.v1.PluginsRestartResponse".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        let descriptor = Self::default().descriptor();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        harmos_protocol::schema::message_types(&types, &Self::name()).expect("generated plugin manager message")
    }
}
impl OperationRequest for wire::PluginsRestartRequest {
    type Response = wire::PluginsRestartResponse;
    const ID: &'static str = "harmos.plugins.restart";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
