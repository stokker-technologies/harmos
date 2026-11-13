/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Constructors and Failure Metadata for Generated Storage Messages.

/* === Implementations === */

impl From<crate::storage::directory_event::Event> for crate::storage::DirectoryEvent {
    fn from(event: crate::storage::directory_event::Event) -> Self {
        Self { event: Some(event) }
    }
}

/* === Public Functions === */

/// The Complete Authoritative Graph, Including Persisted Schema Descriptions.
pub fn storage_types() -> &'static prost_types::FileDescriptorSet {
    use prost::Message;
    static TYPES: std::sync::OnceLock<prost_types::FileDescriptorSet> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| {
        prost_types::FileDescriptorSet::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated storage graph")
    })
}

impl crate::storage::StorageFailure {
    /// Inspect the Existing Operation Failure's Structured Storage Classification.
    pub fn from_operation(failure: &crate::v1::Failure) -> Option<Self> {
        let kind = crate::storage::StorageFailureKind::from_str_name(failure.details.get("storage.kind")?)?;
        Some(Self {
            kind: kind as i32,
            message: failure.message.clone(),
        })
    }
}

impl crate::storage::ArrowPage {
    /// Retained Wire Buffers Include IPC Schema Metadata and Continuations.
    pub fn buffer_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>().saturating_add(self.ipc.capacity());
        bytes = bytes.saturating_add(self.columns_read.capacity().saturating_mul(std::mem::size_of::<String>()));
        for column in &self.columns_read {
            bytes = bytes.saturating_add(column.capacity());
        }
        bytes
    }
}
impl crate::storage::IndexPage {
    pub fn buffer_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>()
            .saturating_add(self.entries.capacity().saturating_mul(std::mem::size_of::<crate::storage::IndexEntry>()));
        for entry in &self.entries {
            bytes = bytes.saturating_add(entry.key.capacity());
        }
        bytes
    }
}
impl crate::storage::ProjectionPage {
    pub fn buffer_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>().saturating_add(self.ipc.capacity());
        bytes = bytes.saturating_add(self.changed_keys.capacity().saturating_mul(std::mem::size_of::<Vec<u8>>()));
        for key in &self.changed_keys {
            bytes = bytes.saturating_add(key.capacity());
        }
        bytes
    }
}
