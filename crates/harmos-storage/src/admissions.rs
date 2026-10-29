/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use super::{RecordInput, RecordWrite, StorageCursor, StorageManager};
use crate::storage::MutationId;
use crate::{Error, Identifier, Result};

#[derive(Clone, Copy)]
pub(super) struct Mutation<'a> {
    pub(super) id: &'a MutationId,
    pub(super) fingerprint: [u8; 32],
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.admissions.write_schema_once", level = "debug", skip_all)]
    pub async fn write_schema_once(
        &self, mutation: &MutationId, key: super::SchemaKey, schema: impl Into<std::sync::Arc<crate::schema::graph::Schema>>,
    ) -> Result<StorageCursor> {
        self.admit_schema(key, schema.into(), Some(mutation)).await
    }

    #[tracing::instrument(name = "storage.admissions.write_records_once", level = "debug", skip_all)]
    pub async fn write_records_once(&self, mutation: &MutationId, records: Vec<RecordInput>) -> Result<StorageCursor> {
        self.admit_record_values(mutation, records).await
    }

    #[tracing::instrument(name = "storage.admissions.admit_record_values", level = "debug", skip_all)]
    pub(crate) async fn admit_record_values(&self, mutation: &MutationId, records: Vec<RecordInput>) -> Result<StorageCursor> {
        nonempty(records.len())?;
        let mut hash = Sha256::new();
        part(&mut hash, b"storage.write_records:v1");
        hash.update((records.len() as u64).to_le_bytes());
        for record in &records {
            hash_record(&mut hash, &record.borrowed());
        }
        self.admit_records(
            records,
            Some(Mutation {
                id: mutation,
                fingerprint: hash.finalize().into(),
            }),
        )
        .await
    }
}

// Called After Shared Schema Preflight Has Bounded The Finite Definition Table.
pub(super) fn schema_mutation<'a>(
    id: &'a MutationId, key: &super::SchemaKey, schema: &crate::schema::graph::Schema,
) -> Result<Mutation<'a>> {
    let mut hash = Sha256::new();
    part(&mut hash, b"storage.write_schema:v1");
    part(&mut hash, key.id.as_str().as_bytes());
    hash.update(key.version.get().to_le_bytes());
    hash.update((schema.root as u64).to_le_bytes());
    hash.update((schema.definitions.len() as u64).to_le_bytes());
    for fields in &schema.definitions {
        hash_fields(&mut hash, fields)?;
    }
    Ok(Mutation { id, fingerprint: hash.finalize().into() })
}
fn hash_fields(hash: &mut Sha256, fields: &[crate::schema::graph::Field]) -> Result<()> {
    use crate::schema::Cardinality;
    use crate::schema::graph::Kind;
    hash.update((fields.len() as u64).to_le_bytes());
    for field in fields {
        if field.name.len() > crate::schema::MAX_NAME_BYTES {
            return Err(Error::InvalidInput {
                field: "storage schema",
                reason: "field name exceeds limit",
            });
        }
        part(hash, field.name.as_bytes());
        hash.update(field.tag.to_le_bytes());
        hash.update([match field.cardinality {
            Cardinality::Required => 0,
            Cardinality::Optional => 1,
            Cardinality::Repeated => 2,
            Cardinality::Variant => 3,
        }]);
        hash.update([match field.kind {
            Kind::Bool => 0,
            Kind::U32 => 1,
            Kind::U64 => 2,
            Kind::I32 => 3,
            Kind::I64 => 4,
            Kind::F32 => 5,
            Kind::F64 => 6,
            Kind::String => 7,
            Kind::Bytes => 8,
            Kind::Message { .. } => 9,
        }]);
        if let Kind::Message { definition } = field.kind {
            hash.update((definition as u64).to_le_bytes());
        }
    }
    Ok(())
}

fn nonempty(count: usize) -> Result<()> {
    if count == 0 {
        return Err(Error::InvalidInput {
            field: "storage mutation",
            reason: "write batch must not be empty",
        });
    }
    Ok(())
}
fn part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn optional(hash: &mut Sha256, value: Option<&Identifier>) {
    hash.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        part(hash, value.as_str().as_bytes());
    }
}
fn hash_record(hash: &mut Sha256, record: &RecordWrite<'_>) {
    part(hash, record.schema.id.as_str().as_bytes());
    hash.update(record.schema.version.get().to_le_bytes());
    optional(hash, record.scope);
    part(hash, record.item.as_str().as_bytes());
    optional(hash, record.source);
    hash.update([u8::from(record.source_time.is_some())]);
    if let Some(time) = record.source_time {
        let (negative, duration) = match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => (false, duration),
            Err(error) => (true, error.duration()),
        };
        hash.update([u8::from(negative)]);
        hash.update(duration.as_secs().to_le_bytes());
        hash.update(duration.subsec_nanos().to_le_bytes());
    }
    part(hash, record.payload);
}
