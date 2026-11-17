/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{self, Write};
use std::time::UNIX_EPOCH;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::{RecordInput, RecordWrite, StorageCursor, StorageManager};
use crate::MutationId;
use crate::{Error, Identifier, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub(super) struct Mutation<'a> {
    pub(super) id: &'a MutationId,
    pub(super) fingerprint: [u8; 32],
}

pub(super) struct Fingerprint(pub(super) Sha256);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    #[tracing::instrument(name = "storage.admissions.write_records_once", level = "debug", skip_all)]
    pub async fn write_records_once(&self, mutation: &MutationId, records: Vec<RecordInput>) -> Result<StorageCursor> {
        self.admit_record_values(mutation, records).await
    }

    #[tracing::instrument(name = "storage.admissions.admit_record_values", level = "debug", skip_all)]
    pub(crate) async fn admit_record_values(&self, mutation: &MutationId, records: Vec<RecordInput>) -> Result<StorageCursor> {
        nonempty(records.len())?;
        let mut hash = Sha256::new();
        part(&mut hash, b"storage.append_records:v1");
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

impl Write for Fingerprint {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

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

/* === Tests === */
