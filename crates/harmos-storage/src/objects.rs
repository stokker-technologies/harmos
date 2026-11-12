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
use std::num::NonZeroUsize;
use std::sync::Arc;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::manager::{Domain, Operation};
use super::{
    ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectValue, ObjectVersion, ObjectVersionToken, StorageManager, StorageSchema,
    StoredObject,
};
use crate::Result;
use harmos_domain::model::storage::catalog::{decode_admitted, object_schema};
use harmos_domain::model::storage::object_key;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ObjectGroup<'a> {
    manager: &'a StorageManager,
    first: u64,
    count: usize,
    allowance: usize,
    used: usize,
    schemas: Vec<StorageSchema>,
    objects: Vec<ObjectEntry>,
    receipts: Vec<ObjectReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectExchange {
    pub expected: Option<ObjectVersionToken>,
    pub schema: StorageSchema,
    pub value: ObjectEntry,
}

struct Fingerprint(Sha256);

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompareExchange {
    Applied(ObjectReceipt),
    Conflict { current: Option<ObjectVersionToken> },
}

/* === Traits === */

/* === Implementations === */

impl ObjectExchange {
    pub fn validate(&self) -> Result<()> {
        self.value.version.check_position(&self.value.position)?;
        self.value.position.history.check(&self.schema.position.history)?;
        if self.schema.position.sequence == 0
            || self.schema.position.sequence.checked_add(1) != Some(self.value.position.sequence)
            || self.schema.key != self.value.value.schema
        {
            return Err(invalid("conditional schema and object must own consecutive matching positions"));
        }
        if let Some(expected) = &self.expected {
            self.value.position.history.check(expected.history())?;
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<[u8; 32]> {
        let mut writer = Fingerprint(Sha256::new());
        writer.0.update(b"storage.object.exchange:v1\0");
        serde_json::to_writer(&mut writer, self)
            .map_err(|source| crate::Error::backend("fingerprint conditional object", crate::BackendFailure::Permanent, source))?;
        Ok(writer.0.finalize().into())
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

impl ObjectGroup<'_> {
    pub fn write<T: StoredObject>(&mut self, value: &T) -> Result<()> {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        let (value, graph) = prepare(self.manager, value)?;
        self.append(value, graph)
    }
}

impl StorageManager {
    pub async fn compare_exchange<T: StoredObject>(&self, expected: Option<ObjectVersionToken>, value: &T) -> Result<CompareExchange> {
        self.admit_exchange(None, expected, value).await
    }
    pub async fn compare_exchange_once<T: StoredObject>(
        &self, mutation: &crate::MutationId, expected: Option<ObjectVersionToken>, value: &T,
    ) -> Result<CompareExchange> {
        self.admit_exchange(Some(mutation), expected, value).await
    }
    pub async fn delete_object_receipts(&self, through: &crate::StorageCursor) -> Result<()> {
        self.flush_until(through).await?;
        self.inner.bounded("primary.delete_object_receipts", self.inner.primary.delete_object_receipts(through)).await
    }

    async fn admit_exchange<T: StoredObject>(
        &self, mutation: Option<&crate::MutationId>, expected: Option<ObjectVersionToken>, value: &T,
    ) -> Result<CompareExchange> {
        let (value, graph) = prepare(self, value)?;
        self.admit_exchange_value(mutation, expected, value, graph).await
    }

    pub async fn compare_exchange_value_once(
        &self, mutation: &crate::MutationId, expected: Option<ObjectVersionToken>, value: ObjectValue, graph: crate::schema::graph::Schema,
    ) -> Result<CompareExchange> {
        self.inner.catalog.validate_value(&value, self.inner.settings.max_batch_bytes())?;
        let definition = self.inner.catalog.definition(&value.schema)?;
        let declared = definition.graph(&harmos_domain::config::SchemaSettings::default())?;
        if graph != declared {
            return Err(invalid("conditional object schema differs from its catalog"));
        }
        self.admit_exchange_value(Some(mutation), expected, value, graph).await
    }

    async fn admit_exchange_value(
        &self, mutation: Option<&crate::MutationId>, expected: Option<ObjectVersionToken>, value: ObjectValue,
        graph: crate::schema::graph::Schema,
    ) -> Result<CompareExchange> {
        if !self.inner.primary.get_storage_capabilities().conditional_objects {
            return Err(crate::Error::backend(
                "conditional objects",
                crate::BackendFailure::Unsupported,
                io::Error::new(io::ErrorKind::Unsupported, "backend has no atomic conditional object primitive"),
            ));
        }
        let _writer = self.inner.object_writes.lock().await;
        let graph = Arc::new(graph);
        if let Some(expected) = &expected {
            self.inner.primary.get_history().check(expected.history())?;
        }
        let fingerprint = if mutation.is_some() {
            let mut writer = Fingerprint(Sha256::new());
            writer.0.update(b"storage.compare_exchange:v1\0");
            serde_json::to_writer(&mut writer, &(&expected, &value))
                .map_err(|source| crate::Error::backend("fingerprint conditional admission", crate::BackendFailure::Permanent, source))?;
            Some(writer.0.finalize().into())
        } else {
            None
        };
        let mutation = mutation.zip(fingerprint).map(|(id, fingerprint)| super::admissions::Mutation { id, fingerprint });
        let mut prepared = Some((value, graph));
        let mut request = None;
        let sequence = self
            .admit_mutation(Domain::Data, self.object_admission_bytes().get(), 2, mutation, false, |first| {
                let (value, graph) = prepared.take().expect("one admission build");
                let position = self.position(first + 1);
                let exchange = Arc::new(ObjectExchange {
                    expected: expected.clone(),
                    schema: StorageSchema::new(self.position(first), value.schema.clone(), graph),
                    value: ObjectEntry {
                        version: ObjectVersionToken::new(position.clone()),
                        position,
                        value,
                    },
                });
                request = Some(exchange.clone());
                Ok(Operation::Conditional(exchange))
            })
            .await?;
        let through = self.position(sequence);
        self.flush_until(&through).await?;
        let request = if let Some(request) = request {
            request
        } else {
            let (value, graph) = prepared.take().expect("replayed admission keeps its input");
            Arc::new(ObjectExchange {
                expected,
                schema: StorageSchema::new(self.position(sequence - 1), value.schema.clone(), graph),
                value: ObjectEntry {
                    version: ObjectVersionToken::new(through.clone()),
                    position: through,
                    value,
                },
            })
        };
        self.inner.bounded("primary.set_object", self.inner.primary.set_object(&request)).await
    }
}

impl ObjectGroup<'_> {
    pub fn remaining_bytes(&self) -> usize {
        self.allowance - self.used
    }

    pub(super) fn write_value(&mut self, value: ObjectValue) -> Result<()> {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        let definition = self.manager.inner.catalog.definition(&value.schema)?;
        let settings = harmos_domain::config::SchemaSettings::default();
        let graph = definition.graph(&settings)?;
        definition.validate_admitted(&value, self.manager.inner.settings.max_batch_bytes(), settings.preparation_bytes.get())?;
        self.append(value, graph)
    }

    fn append(&mut self, value: ObjectValue, graph: crate::schema::graph::Schema) -> Result<()> {
        let sequence = self.first + (self.objects.len() * 2) as u64;
        let descriptor = StorageSchema::new(self.manager.position(sequence), value.schema.clone(), graph);
        let position = self.manager.position(sequence + 1);
        let version = ObjectVersionToken::new(position.clone());
        let object = ObjectEntry {
            position: position.clone(),
            version: version.clone(),
            value,
        };
        let bytes = descriptor
            .encoded_bytes()?
            .checked_add(crate::manager::encoded_size(&object)?)
            .ok_or_else(|| invalid("object size overflow"))?;
        if bytes > self.remaining_bytes() {
            return Err(invalid("object group exceeds its serialized byte limit"));
        }
        self.used += bytes;
        self.schemas.push(descriptor);
        self.objects.push(object);
        self.receipts.push(ObjectReceipt { position, version });
        Ok(())
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.objects.write_object", level = "debug", skip_all)]
    pub async fn write_object<T: StoredObject>(&self, value: &T) -> Result<ObjectReceipt> {
        self.inner.catalog.handle::<T>()?;
        let receipts = self.write_object_group(self.object_admission_bytes(), NonZeroUsize::MIN, |group| group.write(value)).await?;
        Ok(receipts.into_iter().next().unwrap())
    }

    #[tracing::instrument(name = "storage.objects.write_object_group", level = "debug", skip_all)]
    pub async fn write_object_group(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, build: impl FnOnce(&mut ObjectGroup<'_>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        self.admit_object_group(max_bytes, count, None, build).await
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.manager.get_object", level = "debug", skip_all)]
    pub async fn get_object<T: StoredObject>(
        &self, key: &T::Key, version: ObjectVersion,
    ) -> Result<Option<crate::ReadBatch<ObjectRevision<T>>>> {
        self.inner.catalog.handle::<T>()?;
        let schema = object_schema::<T>()?;
        let budget = self.inner.settings.scan_bytes();
        let point = ObjectPoint {
            schema: schema.id.clone(),
            item: object_key(key, budget)?,
            version,
        };
        let saved = self.get_saved_position();
        if let ObjectVersion::Exact(version) = &point.version {
            version.history().check(&saved.history)?;
            if version.revision().get() > saved.sequence {
                return Ok(None);
            }
        }

        let snapshot = self.snapshot(&saved).await?;
        let Some(object) = snapshot.get_object(&point).await? else {
            return Ok(None);
        };
        if matches!(&point.version, ObjectVersion::Exact(version) if version != &object.version) {
            return Err(invalid("backend returned a different object revision"));
        }
        let value = decode_admitted::<T>(&object.value, &schema)?;
        let revision = ObjectRevision {
            value,
            position: object.position,
            version: object.version,
        };
        Ok(Some(crate::ReadBatch::new(revision)))
    }
}

impl StorageManager {
    pub(super) fn object_admission_bytes(&self) -> NonZeroUsize {
        NonZeroUsize::new(
            self.inner.settings.max_batch_bytes().get().saturating_mul(4).min(self.inner.settings.request.max_pending_write_bytes.get()),
        )
        .unwrap()
    }

    #[tracing::instrument(name = "storage.objects.admit_object_group", level = "debug", skip_all)]
    pub(super) async fn admit_object_group(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        let _writer = tracing::Instrument::instrument(self.inner.object_writes.lock(), tracing::trace_span!("storage.lock.wait")).await;
        self.admit_object_group_locked(max_bytes, count, mutation, build).await
    }

    #[tracing::instrument(name = "storage.objects.admit_object_group_locked", level = "debug", skip_all)]
    pub(super) async fn admit_object_group_locked(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        let positions = count.get().checked_mul(2).ok_or_else(|| invalid("object group count overflow"))?;
        let mut receipts = None;
        let last = self
            .admit_mutation(Domain::Data, max_bytes.get(), positions, mutation, false, |first| {
                let mut group = ObjectGroup {
                    manager: self,
                    first,
                    count: count.get(),
                    allowance: max_bytes.get(),
                    used: 0,
                    schemas: Vec::with_capacity(count.get()),
                    objects: Vec::with_capacity(count.get()),
                    receipts: Vec::with_capacity(count.get()),
                };
                build(&mut group)?;
                if group.objects.len() != count.get() {
                    return Err(invalid("object group count differs from admission"));
                }
                receipts = Some(group.receipts);
                Ok(Operation::ObjectGroup {
                    schemas: super::StorageBatch::new(group.schemas),
                    objects: super::StorageBatch::new(group.objects),
                    bytes: group.used,
                })
            })
            .await?;
        if let Some(receipts) = receipts {
            return Ok(receipts);
        }
        // A Matching Retry Reconstructs the Same Revision Tokens from Its Fixed
        // Original Group Frontier. No Payload Is Revalidated Or Readmitted.
        let first = last.checked_sub(positions as u64 - 2).ok_or_else(|| invalid("invalid object group receipt"))?;
        let mut receipts = Vec::with_capacity(count.get());
        for index in 0..count.get() {
            let position = self.position(first + (index * 2) as u64);
            receipts.push(ObjectReceipt {
                version: ObjectVersionToken::new(position.clone()),
                position,
            });
        }
        Ok(receipts)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn prepare<T: StoredObject>(manager: &StorageManager, value: &T) -> Result<(ObjectValue, crate::schema::graph::Schema)> {
    manager.inner.catalog.handle::<T>()?;
    let graph = crate::schema::graph::Schema::from_declaration(T::DEFINITION, &harmos_domain::config::SchemaSettings::default())
        .map_err(|_| invalid("invalid object schema"))?;
    let schema = object_schema::<T>()?;
    let item = object_key(value.key(), manager.inner.settings.max_batch_bytes())?;
    let payload = value.encode();
    if payload.len() != value.encoded_size() {
        return Err(invalid("object codec length differs"));
    }
    let value = ObjectValue { schema, item, payload };
    manager.inner.catalog.validate_value(&value, manager.inner.settings.max_batch_bytes())?;
    Ok((value, graph))
}

pub(super) fn invalid(reason: &'static str) -> crate::Error {
    harmos_domain::model::storage::invalid(reason).into()
}

/* === Tests === */
