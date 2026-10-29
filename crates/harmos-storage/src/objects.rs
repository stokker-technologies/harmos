/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::NonZeroUsize;

use super::manager::{Domain, Operation};
use super::{
    ObjectCatalog, ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectValue, ObjectVersion, ObjectVersionToken, StorageManager,
    StorageSchema, StoredObject,
};
use crate::Result;
use harmos_domain::catalog::Contains;
use harmos_domain::model::storage::catalog::{decode_admitted, object_schema};
use harmos_domain::model::storage::object_key;

pub struct ObjectGroup<'a, C> {
    manager: &'a StorageManager<C>,
    first: u64,
    count: usize,
    allowance: usize,
    used: usize,
    schemas: Vec<StorageSchema>,
    objects: Vec<ObjectEntry>,
    receipts: Vec<ObjectReceipt>,
}

impl<C: ObjectCatalog> ObjectGroup<'_, C> {
    pub fn write<T: StoredObject>(&mut self, value: &T) -> Result<()>
    where
        C: Contains<T>,
    {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        let settings = harmos_domain::config::SchemaSettings::default();
        let graph =
            crate::schema::graph::Schema::from_declaration(T::DEFINITION, &settings).map_err(|_| invalid("invalid object schema"))?;
        let schema = object_schema::<T>()?;
        let item = object_key(value.key(), self.manager.inner.settings.max_batch_bytes())?;
        let payload = value.encode();
        if payload.len() != value.encoded_size() {
            return Err(invalid("object codec length differs"));
        }
        self.append(ObjectValue { schema, item, payload }, graph)
    }
}

impl<C> ObjectGroup<'_, C> {
    pub fn remaining_bytes(&self) -> usize {
        self.allowance - self.used
    }

    pub(super) fn write_value(&mut self, value: ObjectValue) -> Result<()> {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        let definition = (self.manager.inner.object_definition)(&value.schema)?;
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

impl<C: ObjectCatalog> StorageManager<C> {
    #[tracing::instrument(name = "storage.objects.write_object", level = "debug", skip_all)]
    pub async fn write_object<T: StoredObject>(&self, value: &T) -> Result<ObjectReceipt>
    where
        C: Contains<T>,
    {
        let receipts = self.write_object_group(self.object_admission_bytes(), NonZeroUsize::MIN, |group| group.write(value)).await?;
        Ok(receipts.into_iter().next().unwrap())
    }

    #[tracing::instrument(name = "storage.objects.write_object_group", level = "debug", skip_all)]
    pub async fn write_object_group(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        self.admit_object_group(max_bytes, count, None, build).await
    }
}

impl<C: ObjectCatalog> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.read_object", level = "debug", skip_all)]
    pub async fn read_object<T: StoredObject>(
        &self, key: &T::Key, version: ObjectVersion,
    ) -> Result<Option<crate::ReadBatch<ObjectRevision<T>>>>
    where
        C: Contains<T>,
    {
        let schema = object_schema::<T>()?;
        let budget = self.inner.settings.scan_bytes();
        let point = ObjectPoint {
            schema: schema.id.clone(),
            item: object_key(key, budget)?,
            version,
        };
        let saved = self.cursor_saved();
        if let ObjectVersion::Exact(version) = &point.version {
            version.history().check(&saved.history)?;
            if version.revision().get() > saved.sequence {
                return Ok(None);
            }
        }

        let snapshot = self.snapshot(&saved).await?;
        let Some(object) = snapshot.read_object(&point).await? else {
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

impl<C> StorageManager<C> {
    pub(super) fn object_admission_bytes(&self) -> NonZeroUsize {
        NonZeroUsize::new(
            self.inner.settings.max_batch_bytes().get().saturating_mul(4).min(self.inner.settings.request.max_pending_write_bytes.get()),
        )
        .unwrap()
    }

    #[tracing::instrument(name = "storage.objects.admit_object_group", level = "debug", skip_all)]
    pub(super) async fn admit_object_group(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        let _writer = tracing::Instrument::instrument(self.inner.object_writes.lock(), tracing::trace_span!("storage.lock.wait")).await;
        self.admit_object_group_locked(max_bytes, count, mutation, build).await
    }

    #[tracing::instrument(name = "storage.objects.admit_object_group_locked", level = "debug", skip_all)]
    pub(super) async fn admit_object_group_locked(
        &self, max_bytes: NonZeroUsize, count: NonZeroUsize, mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
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
        // A Matching Retry Reconstructs The Same Revision Tokens From Its Fixed
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

pub(super) fn invalid(reason: &'static str) -> crate::Error {
    harmos_domain::model::storage::invalid(reason).into()
}
