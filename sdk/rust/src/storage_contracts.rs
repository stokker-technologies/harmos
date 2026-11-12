/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Operation Contracts

// Storage Requests Share the SDK's Managed Invocation Lifecycle.

/* === Imports === */

use crate::{Message, OperationRequest};
use harmos_protocol::storage;
use std::sync::OnceLock;

/* === Implementations === */

impl Message for storage::AdvanceProjection {
    fn name() -> String {
        "harmos.storage.v1.AdvanceProjection".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ArrowPage {
    fn name() -> String {
        "harmos.storage.v1.ArrowPage".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::Capabilities {
    fn name() -> String {
        "harmos.storage.v1.Capabilities".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::CompareExchangeObject {
    fn name() -> String {
        "harmos.storage.v1.CompareExchangeObject".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::CompareExchangeOutcome {
    fn name() -> String {
        "harmos.storage.v1.CompareExchangeOutcome".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::DefinitionRevision {
    fn name() -> String {
        "harmos.storage.v1.DefinitionRevision".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::Empty {
    fn name() -> String {
        "harmos.storage.v1.Empty".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::IndexPage {
    fn name() -> String {
        "harmos.storage.v1.IndexPage".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::InspectProjection {
    fn name() -> String {
        "harmos.storage.v1.InspectProjection".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::OpenSnapshot {
    fn name() -> String {
        "harmos.storage.v1.OpenSnapshot".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ProjectionPage {
    fn name() -> String {
        "harmos.storage.v1.ProjectionPage".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ProjectionState {
    fn name() -> String {
        "harmos.storage.v1.ProjectionState".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ReadProjection {
    fn name() -> String {
        "harmos.storage.v1.ReadProjection".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::RegisterIndex {
    fn name() -> String {
        "harmos.storage.v1.RegisterIndex".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::RegisterProjection {
    fn name() -> String {
        "harmos.storage.v1.RegisterProjection".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ReleaseSnapshot {
    fn name() -> String {
        "harmos.storage.v1.ReleaseSnapshot".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::SavedSnapshot {
    fn name() -> String {
        "harmos.storage.v1.SavedSnapshot".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ScanArrow {
    fn name() -> String {
        "harmos.storage.v1.ScanArrow".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::ScanIndex {
    fn name() -> String {
        "harmos.storage.v1.ScanIndex".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl Message for storage::StorageCapabilities {
    fn name() -> String {
        "harmos.storage.v1.StorageCapabilities".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
        TYPES
            .get_or_init(|| {
                harmos_protocol::schema::message_types(harmos_protocol::storage_types(), &Self::name()).expect("generated storage message")
            })
            .clone()
    }
}
impl OperationRequest for storage::Capabilities {
    type Response = storage::StorageCapabilities;
    const ID: &'static str = "storage.capabilities";
}
impl OperationRequest for storage::OpenSnapshot {
    type Response = storage::SavedSnapshot;
    const ID: &'static str = "storage.open_snapshot";
}
impl OperationRequest for storage::ReleaseSnapshot {
    type Response = storage::Empty;
    const ID: &'static str = "storage.release_snapshot";
}
impl OperationRequest for storage::ScanArrow {
    type Response = storage::ArrowPage;
    const ID: &'static str = "storage.scan_arrow";
}
impl OperationRequest for storage::ScanIndex {
    type Response = storage::IndexPage;
    const ID: &'static str = "storage.scan_index";
}
impl OperationRequest for storage::RegisterIndex {
    type Response = storage::DefinitionRevision;
    const ID: &'static str = "storage.register_index";
}
impl OperationRequest for storage::RegisterProjection {
    type Response = storage::DefinitionRevision;
    const ID: &'static str = "storage.register_projection";
}
impl OperationRequest for storage::AdvanceProjection {
    type Response = storage::DefinitionRevision;
    const ID: &'static str = "storage.advance_projection";
}
impl OperationRequest for storage::ReadProjection {
    type Response = storage::ProjectionPage;
    const ID: &'static str = "storage.read_projection";
}
impl OperationRequest for storage::InspectProjection {
    type Response = storage::ProjectionState;
    const ID: &'static str = "storage.inspect_projection";
}
impl OperationRequest for storage::CompareExchangeObject {
    type Response = storage::CompareExchangeOutcome;
    const ID: &'static str = "storage.compare_exchange_object";
}
