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
use std::num::NonZeroUsize;
// Third Party Imports
// Local Imports
use super::manager::{Domain, ENTRY_OVERHEAD, Operation, encoded_size};
use super::query::{execution_timeout, remaining_budget};
use super::{
    ObjectCatalog, ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectValue,
    ObjectVersion, ObjectVersionToken, QueryValue, StorageCursor, StorageManager, StorageSchema,
    StorageView, StoredObject,
};
use crate::catalog::Contains;
use crate::storage::catalog::{check_length, decode_admitted, invalid, object_key, object_schema};
use crate::{Error, MemorySize, MessageError, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
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

/* === Enums === */
/* === Traits === */

/* === Implementations === */
impl<C: ObjectCatalog> ObjectGroup<'_, C> {
    pub fn remaining_bytes(&self) -> usize {
        self.allowance - self.used
    }

    pub fn write<T: StoredObject>(&mut self, value: &T) -> Result<()>
    where
        C: Contains<T>,
    {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        let settings = self.schema_settings()?;
        let graph = crate::schema::graph::Schema::from_declaration(T::DEFINITION, &settings)
            .map_err(|_| invalid("invalid object schema or preparation allowance"))?;
        let (id, _) = T::definition();
        if graph
            .memory_size()
            .checked_add(id.as_str().len())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<super::SchemaKey>()))
            .is_none_or(|bytes| bytes > self.remaining_bytes())
        {
            return Err(invalid("object identity exceeds allowance"));
        }
        let schema = object_schema::<T>()?;
        let metadata = self.metadata(&schema, &graph)?;
        if metadata >= self.remaining_bytes() {
            return Err(invalid("object metadata exceeds allowance"));
        }
        let limit = NonZeroUsize::new((self.remaining_bytes() - metadata) / 2)
            .ok_or_else(|| invalid("object key exceeds allowance"))?;
        let item = object_key(value.key(), limit)?;
        let key_bytes = item.memory_size();
        let length = value.encoded_len();
        let bytes = metadata
            .checked_add(key_bytes)
            .and_then(|bytes| bytes.checked_add(length))
            .ok_or_else(|| invalid("object size overflow"))?;
        if bytes > self.remaining_bytes() {
            return Err(invalid("object payload exceeds allowance"));
        }
        let payload = value.encode_to_vec();
        if payload.len() != length
            || payload.capacity() > self.remaining_bytes() - metadata - key_bytes
        {
            return Err(invalid("object codec exceeded its reported allowance"));
        }
        self.append(
            ObjectValue {
                schema,
                item,
                payload,
            },
            graph,
            metadata + key_bytes,
        )?;
        Ok(())
    }
}

impl<C> ObjectGroup<'_, C> {
    pub(super) fn write_value(&mut self, value: &ObjectValue) -> Result<()> {
        if self.objects.len() == self.count {
            return Err(invalid("object group contains too many objects"));
        }
        // Catalog Identity Construction And Verification Share The Original Claim.
        if self.allowance - self.used < crate::storage::catalog::LOOKUP_MEMORY_BYTES {
            return Err(invalid("object catalog lookup exceeds allowance"));
        }
        let definition = (self.manager.inner.object_definition)(&value.schema)?;
        let graph = definition.graph(&self.schema_settings()?)?;
        let metadata = self.metadata(&value.schema, &graph)?;
        let verification = (self.allowance - self.used)
            .checked_sub(metadata)
            .ok_or_else(|| invalid("object metadata exceeds allowance"))?;
        definition.validate_admitted(
            value,
            self.manager.inner.settings.max_batch_bytes,
            verification,
        )?;
        let retained = metadata
            .checked_add(value.item.memory_size())
            .and_then(|bytes| bytes.checked_add(value.payload.len()))
            .ok_or_else(|| invalid("object size overflow"))?;
        if retained > self.allowance - self.used {
            return Err(invalid("object payload exceeds allowance"));
        }
        let value = value.clone();
        let bytes = metadata + value.item.memory_size();
        self.append(value, graph, bytes)
    }

    fn schema_settings(&self) -> Result<harmos_contract::config::SchemaSettings> {
        let mut settings = harmos_contract::config::SchemaSettings::default();
        settings.preparation_bytes = NonZeroUsize::new(
            settings
                .preparation_bytes
                .get()
                .min(self.allowance - self.used),
        )
        .ok_or_else(|| invalid("object schema preparation exceeds allowance"))?;
        Ok(settings)
    }

    fn metadata(
        &self,
        schema: &super::SchemaKey,
        graph: &crate::schema::graph::Schema,
    ) -> Result<usize> {
        let descriptor =
            StorageSchema::resident_bytes(schema, graph, self.manager.inner.primary.history())?;
        let clone = graph
            .clone_memory_bytes()
            .map_err(|_| invalid("object schema size overflow"))?;
        let retained = graph
            .memory_size()
            .checked_sub(clone)
            .ok_or_else(|| invalid("object schema clone exceeds retained allocation"))?;
        let identity = self
            .manager
            .charge(&(schema, self.manager.inner.primary.history()), 4)?;
        descriptor
            .checked_add(retained)
            .and_then(|bytes| bytes.checked_add(identity))
            .ok_or_else(|| invalid("object size overflow"))
    }

    fn append(
        &mut self,
        value: ObjectValue,
        graph: crate::schema::graph::Schema,
        metadata: usize,
    ) -> Result<()> {
        let bytes = metadata
            .checked_add(value.payload.capacity())
            .ok_or_else(|| invalid("object size overflow"))?;
        if bytes > self.allowance - self.used {
            return Err(invalid("object payload exceeds allowance"));
        }
        let sequence = self.first + (self.objects.len() * 2) as u64;
        let descriptor =
            StorageSchema::new(self.manager.position(sequence), value.schema.clone(), graph);
        let position = self.manager.position(sequence + 1);
        let version = ObjectVersionToken::new(position.clone());
        self.used += bytes;
        self.schemas.push(descriptor);
        self.objects.push(ObjectEntry {
            position: position.clone(),
            version: version.clone(),
            value,
        });
        self.receipts.push(ObjectReceipt { position, version });
        Ok(())
    }
}

impl<C: ObjectCatalog> StorageManager<C> {
    pub async fn write_object<T: StoredObject>(&self, value: &T) -> Result<ObjectReceipt>
    where
        C: Contains<T>,
    {
        let receipts = self
            .write_object_group(self.object_admission_bytes(), NonZeroUsize::MIN, |group| {
                group.write(value)
            })
            .await?;
        Ok(receipts.into_iter().next().unwrap())
    }

    pub async fn write_object_group(
        &self,
        max_bytes: NonZeroUsize,
        count: NonZeroUsize,
        build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        self.admit_object_group(max_bytes, count, None, build).await
    }

    pub async fn read_object<T: StoredObject>(
        &self,
        key: &T::Key,
        version: ObjectVersion,
        max_bytes: NonZeroUsize,
    ) -> Result<Option<QueryValue<ObjectRevision<T>>>>
    where
        C: Contains<T>,
    {
        self.read_object_at(None, key, version, max_bytes).await
    }

    pub async fn read_object_in<T: StoredObject>(
        &self,
        view: &StorageView,
        key: &T::Key,
        version: ObjectVersion,
        max_bytes: NonZeroUsize,
    ) -> Result<Option<QueryValue<ObjectRevision<T>>>>
    where
        C: Contains<T>,
    {
        self.read_object_at(Some(view), key, version, max_bytes)
            .await
    }

    async fn read_object_at<T: StoredObject>(
        &self,
        view: Option<&StorageView>,
        key: &T::Key,
        version: ObjectVersion,
        max_bytes: NonZeroUsize,
    ) -> Result<Option<QueryValue<ObjectRevision<T>>>>
    where
        C: Contains<T>,
    {
        let started = tokio::time::Instant::now();
        // A Supplied View Lends Its Own Budget; Standalone Reads Use The Defaults.
        let execution = match view {
            Some(view) => {
                if !self.same_instance(view.manager()) {
                    return Err(Error::InvalidInput {
                        field: "query view",
                        reason: "view belongs to another manager",
                    });
                }
                view.inherited_budget(None)?
            }
            None => self.inner.settings.query_execution,
        };
        self.inner
            .settings
            .query_page_ceiling
            .check(1, max_bytes.get())?;
        let schema = object_schema::<T>()?;
        let query = ObjectPoint {
            schema: schema.id.clone(),
            item: object_key(key, max_bytes)?,
            version,
        };
        let bytes = self.inner.settings.query_decoded_bytes;
        let reservation = self.reserve_read_now(bytes.get(), 1)?;
        let timeout = remaining_budget(execution.timeout, started)?;
        tokio::time::timeout(timeout, async {
            let view = match view {
                Some(view) => view.clone(),
                None => self.view_with_limits(execution).await?,
            };
            view.limit_computation(remaining_budget(execution.timeout, started)?)?;
            view.compute(view.read_object_mapped(&query, max_bytes, |object| {
                let value = object
                    .map(|object| {
                        // Preflight Native Decoding Within The Original Output Reservation.
                        let metadata = std::mem::size_of::<ObjectRevision<T>>()
                            .saturating_sub(std::mem::size_of::<T>())
                            .checked_add(
                                object
                                    .position
                                    .memory_size()
                                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
                            )
                            .and_then(|size| {
                                size.checked_add(
                                    object
                                        .version
                                        .memory_size()
                                        .saturating_sub(std::mem::size_of::<ObjectVersionToken>()),
                                )
                            })
                            .and_then(|size| size.checked_add(ENTRY_OVERHEAD))
                            .and_then(|size| size.checked_add(std::mem::size_of::<MessageError>()))
                            .ok_or_else(super::query::decoded_limit)?;
                        let allowance = bytes
                            .get()
                            .checked_sub(metadata)
                            .ok_or_else(super::query::decoded_limit)?;
                        let required =
                            T::decoding_memory(&object.value.payload).map_err(|error| {
                                Error::backend(
                                    "preflight object",
                                    crate::BackendFailure::Corrupt,
                                    error,
                                )
                            })?;
                        if required > allowance {
                            return Err(super::query::decoded_limit());
                        }
                        let value = ObjectRevision {
                            value: decode_admitted::<T>(&object.value, &schema)?,
                            position: object.position,
                            version: object.version,
                        };
                        QueryValue::retain(value, reservation)
                    })
                    .transpose()?;
                if started.elapsed() >= execution.timeout {
                    return Err(execution_timeout());
                }
                Ok(value)
            }))
            .await
        })
        .await
        .map_err(|_| execution_timeout())?
    }
}

impl<C> StorageManager<C> {
    pub(super) fn object_admission_bytes(&self) -> NonZeroUsize {
        NonZeroUsize::new(
            self.inner
                .settings
                .max_batch_bytes
                .get()
                .saturating_mul(4)
                .min(self.inner.settings.request.max_pending_write_bytes.get()),
        )
        .unwrap()
    }

    pub(super) async fn admit_object_group(
        &self,
        max_bytes: NonZeroUsize,
        count: NonZeroUsize,
        mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        let _writer = self.inner.object_writes.lock().await;
        self.admit_object_group_locked(max_bytes, count, mutation, build)
            .await
    }

    pub(super) async fn admit_object_group_locked(
        &self,
        max_bytes: NonZeroUsize,
        count: NonZeroUsize,
        mutation: Option<super::admissions::Mutation<'_>>,
        build: impl FnOnce(&mut ObjectGroup<'_, C>) -> Result<()> + Send,
    ) -> Result<Vec<ObjectReceipt>> {
        let positions = count
            .get()
            .checked_mul(2)
            .ok_or_else(|| invalid("object group count overflow"))?;
        let mut receipts = None;
        let last = self
            .admit_mutation(
                Domain::Data,
                max_bytes.get(),
                positions,
                mutation,
                false,
                |first| {
                    let slots = count
                        .get()
                        .checked_mul(
                            std::mem::size_of::<StorageSchema>()
                                + std::mem::size_of::<ObjectEntry>()
                                + std::mem::size_of::<ObjectReceipt>()
                                + 2 * std::mem::size_of::<super::StoragePart<'_>>()
                                + 2 * ENTRY_OVERHEAD,
                        )
                        .ok_or_else(|| invalid("object group size overflow"))?;
                    if slots > max_bytes.get() {
                        return Err(invalid("object group metadata exceeds allowance"));
                    }
                    let mut group = ObjectGroup {
                        manager: self,
                        first,
                        count: count.get(),
                        allowance: max_bytes.get(),
                        used: slots,
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
                        schemas: group.schemas,
                        objects: group.objects,
                        bytes: group.used,
                    })
                },
            )
            .await?;
        if let Some(receipts) = receipts {
            return Ok(receipts);
        }
        // A Matching Retry Reconstructs The Same Revision Tokens From Its Fixed
        // Original Group Frontier. No Payload Is Revalidated Or Readmitted.
        let first = last
            .checked_sub(positions as u64 - 2)
            .ok_or_else(|| invalid("invalid object group receipt"))?;
        Ok((0..count.get())
            .map(|index| {
                let position = self.position(first + (index * 2) as u64);
                ObjectReceipt {
                    version: ObjectVersionToken::new(position.clone()),
                    position,
                }
            })
            .collect())
    }
}

impl StorageView {
    pub(super) async fn read_object_mapped<T>(
        &self,
        query: &ObjectPoint,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(Option<ObjectEntry>) -> Result<T>,
    ) -> Result<T> {
        check_length(query.item.as_str().len(), max_bytes)?;
        let _activity = self.activity()?;
        let manager = self.manager();
        let _lifecycle = manager.inner.lifecycle.lock().await;
        let metadata = encoded_size(query)?
            .checked_add(encoded_size(manager.inner.primary.history())?)
            .and_then(|n| n.checked_add(ENTRY_OVERHEAD))
            .and_then(|n| n.checked_mul(3))
            .and_then(|n| n.checked_add(max_bytes.get()))
            .ok_or_else(|| invalid("object read size overflow"))?;
        let _reservation = manager.reserve_read(metadata, 1).await?;
        let (_pin, view) = self.pin()?;
        if let ObjectVersion::Exact(version) = &query.version {
            version.history.check(&view.saved().history)?;
            if version.revision.get() > view.saved().sequence {
                drop(_lifecycle);
                return map(None);
            }
        }
        let object = manager
            .inner
            .bounded(manager.inner.primary.read_object(&view, query, max_bytes))
            .await?;
        if let Some(object) = &object {
            check_length(encoded_size(object)?, max_bytes)?;
            object.version.check_position(&object.position)?;
            if object.position.sequence == 0
                || !view.saved().covers(&object.position)?
                || object.value.schema.id != query.schema
                || object.value.item != query.item
                || matches!(&query.version, ObjectVersion::Exact(version) if version != &object.version)
            {
                return Err(invalid("backend returned a different object revision"));
            }
            // The Typed Mapper Verifies Schema, Payload And Key After Its Preclaim.
        }
        let _still_live = self.pin()?;
        drop(_lifecycle);
        map(object)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
