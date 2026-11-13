/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Storage Protocol Conversion and IPC Validation.

/* === Imports === */

use super::{
    columnar::{ArrowPage, ColumnKind, ColumnSource, StorageCapabilities},
    index::{IndexDefinition, IndexPage},
    projection::{ProjectionDefinition, ProjectionPage, ProjectionStatus, ProjectionTransform},
};
use crate::{Error, ReadLimits, Result};
use harmos_protocol::storage as wire;

/* === Implementations === */

impl TryFrom<wire::ReadLimits> for ReadLimits {
    type Error = Error;
    fn try_from(v: wire::ReadLimits) -> Result<Self> {
        Ok(Self {
            rows: nonzero(v.rows)?,
            bytes: nonzero(v.bytes)?,
        })
    }
}
impl From<ReadLimits> for wire::ReadLimits {
    fn from(v: ReadLimits) -> Self {
        Self {
            rows: v.rows.get() as u64,
            bytes: v.bytes.get() as u64,
        }
    }
}
impl From<StorageCapabilities> for wire::StorageCapabilities {
    fn from(v: StorageCapabilities) -> Self {
        Self {
            native_arrow: v.native_arrow,
            native_index: v.native_index,
            durable_projections: v.durable_projections,
            conditional_objects: v.conditional_objects,
            file_sessions: v.file_sessions,
            file_deletion: v.file_deletion,
            mutation_receipts: v.mutation_receipts,
        }
    }
}
impl TryFrom<wire::ColumnSource> for ColumnSource {
    type Error = Error;
    fn try_from(v: wire::ColumnSource) -> Result<Self> {
        let kind = match wire::ColumnKind::try_from(v.kind).map_err(|_| invalid("unknown column kind"))? {
            wire::ColumnKind::Objects => ColumnKind::Objects,
            wire::ColumnKind::Records => ColumnKind::Records,
            wire::ColumnKind::Stream => ColumnKind::Stream,
        };
        Ok(Self {
            kind,
            schema: required(v.schema)?.try_into()?,
        })
    }
}
impl From<ColumnSource> for wire::ColumnSource {
    fn from(v: ColumnSource) -> Self {
        Self {
            kind: match v.kind {
                ColumnKind::Objects => wire::ColumnKind::Objects as i32,
                ColumnKind::Records => wire::ColumnKind::Records as i32,
                ColumnKind::Stream => wire::ColumnKind::Stream as i32,
            },
            schema: Some(v.schema.into()),
        }
    }
}
impl TryFrom<ArrowPage> for wire::ArrowPage {
    type Error = Error;
    fn try_from(v: ArrowPage) -> Result<Self> {
        Ok(Self {
            ipc: crate::arrow::ArrowBatch::try_new(v.batch)?.ipc_bytes().to_vec(),
            complete: v.complete,
            scanned: v.scanned as u64,
            bytes_read: v.bytes_read as u64,
            columns_read: v.columns_read,
            payload_rows: v.payload_rows as u64,
        })
    }
}
impl TryFrom<wire::IndexDefinition> for IndexDefinition {
    type Error = Error;
    fn try_from(v: wire::IndexDefinition) -> Result<Self> {
        let definition = Self {
            name: crate::Identifier::new(v.name).map_err(|_| invalid("invalid storage name"))?,
            version: v.version,
            source: required(v.source)?.try_into()?,
            columns: v.columns,
        };
        definition.validate()?;
        Ok(definition)
    }
}
impl From<IndexPage> for wire::IndexPage {
    fn from(v: IndexPage) -> Self {
        Self {
            entries: v.entries.into_iter().map(|e| wire::IndexEntry { key: e.key, position: e.position }).collect(),
            complete: v.complete,
        }
    }
}
impl TryFrom<wire::ProjectionDefinition> for ProjectionDefinition {
    type Error = Error;
    fn try_from(v: wire::ProjectionDefinition) -> Result<Self> {
        let transform = match required(v.transform)? {
            wire::projection_definition::Transform::Select(s) => ProjectionTransform::Select(s.columns),
            wire::projection_definition::Transform::TimeBuckets(t) => ProjectionTransform::TimeBuckets(t.try_into()?),
        };
        let definition = Self {
            name: crate::Identifier::new(v.name).map_err(|_| invalid("invalid storage name"))?,
            version: v.version,
            source: required(v.source)?.try_into()?,
            transform,
        };
        definition.validate()?;
        Ok(definition)
    }
}
impl TryFrom<ProjectionPage> for wire::ProjectionPage {
    type Error = Error;
    fn try_from(v: ProjectionPage) -> Result<Self> {
        Ok(Self {
            ipc: crate::arrow::ArrowBatch::try_new(v.batch)?.ipc_bytes().to_vec(),
            changed_keys: v.changed_keys,
        })
    }
}
impl From<ProjectionStatus> for wire::ProjectionStatus {
    fn from(v: ProjectionStatus) -> Self {
        Self {
            frontier: Some(v.frontier.into()),
            revision: v.revision,
            rows: v.rows as u64,
            changed_keys: v.changed_keys as u64,
        }
    }
}
/* === Public Functions === */

pub fn required<T>(v: Option<T>) -> Result<T> {
    v.ok_or_else(|| invalid("required storage field is absent"))
}

pub fn decode_batch(bytes: Vec<u8>, limits: ReadLimits) -> Result<crate::arrow::ArrowBatch> {
    if bytes.len() > limits.bytes.get() {
        return Err(Error::ResourceExhausted { resource: "Arrow IPC bytes" });
    }
    let batch = crate::arrow::ArrowBatch::from_ipc(bytes)?;
    if batch.as_record_batch().num_rows() > limits.rows.get() || batch.as_record_batch().get_array_memory_size() > limits.bytes.get() {
        return Err(Error::ResourceExhausted { resource: "Arrow page" });
    }
    Ok(batch)
}

/* === Private Functions === */

fn nonzero(v: u64) -> Result<std::num::NonZeroUsize> {
    usize::try_from(v)
        .ok()
        .and_then(std::num::NonZeroUsize::new)
        .ok_or_else(|| invalid("read limits must be nonzero and fit this platform"))
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage protocol", reason }
}

impl From<IndexDefinition> for wire::IndexDefinition {
    fn from(v: IndexDefinition) -> Self {
        Self {
            name: v.name.to_string(),
            version: v.version,
            source: Some(v.source.into()),
            columns: v.columns,
        }
    }
}
impl TryFrom<ProjectionDefinition> for wire::ProjectionDefinition {
    type Error = Error;
    fn try_from(v: ProjectionDefinition) -> Result<Self> {
        Ok(Self {
            name: v.name.to_string(),
            version: v.version,
            source: Some(v.source.into()),
            transform: Some(match v.transform {
                ProjectionTransform::Select(columns) => wire::projection_definition::Transform::Select(wire::SelectColumns { columns }),
                ProjectionTransform::TimeBuckets(plan) => wire::projection_definition::Transform::TimeBuckets(plan.try_into()?),
            }),
        })
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_plans_refuse_before_backend_access() {
        assert!(ReadLimits::try_from(wire::ReadLimits { rows: 0, bytes: 64 }).is_err());
        assert!(ReadLimits::try_from(wire::ReadLimits { rows: 1, bytes: 0 }).is_err());
        let limits = ReadLimits {
            rows: std::num::NonZeroUsize::MIN,
            bytes: std::num::NonZeroUsize::new(64).unwrap(),
        };
        assert!(decode_batch(vec![0; 65], limits).is_err());
        assert!(decode_batch(vec![0; 32], limits).is_err());
        let plan = wire::TimeBucketPlan {
            width_ns: 0,
            max_samples: 1,
            max_bytes: 64,
            ..Default::default()
        };
        assert!(super::super::time_buckets::TimeBucketPlan::try_from(plan).is_err());
    }
}
