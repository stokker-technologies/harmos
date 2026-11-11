/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Schematic Messages Shared by Native Handlers and SDK Clients.

#[rustfmt::skip]
#[path = "messages.rs"]
mod messages;
pub use messages::*;
#[rustfmt::skip]
#[path = "reflection.rs"]
mod reflection;
fn pool() -> &'static prost_reflect::DescriptorPool {
    static POOL: std::sync::OnceLock<prost_reflect::DescriptorPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        prost_reflect::DescriptorPool::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated schematic descriptors")
    })
}

#[cfg(feature = "sdk")]
fn descriptors() -> &'static prost_types::FileDescriptorSet {
    static TYPES: std::sync::OnceLock<prost_types::FileDescriptorSet> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| prost::Message::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated schematic graph"))
}

#[cfg(feature = "sdk")]
impl harmos::OperationRequest for ValidateSchematic {
    type Response = Validation;
    const ID: &'static str = "schematic.validate";
}
#[cfg(feature = "sdk")]
impl harmos::OperationRequest for ProposeRoute {
    type Response = Route;
    const ID: &'static str = "schematic.route";
}
#[cfg(feature = "sdk")]
impl harmos::OperationRequest for ListDirectories {
    type Response = Directories;
    const ID: &'static str = "schematic.list.directories";
}
