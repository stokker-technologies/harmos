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
impl OperationRequest for storage::CompareExchangeObject {
    type Response = storage::CompareExchangeOutcome;
    const ID: &'static str = "storage.compare_exchange_object";
}
