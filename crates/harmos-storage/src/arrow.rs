/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

/* === Modules === */

mod ipc;
mod retention;

/* === Re-Exports === */

pub use super::writes::convert::{encode_payloads, encode_payloads_memory};

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, Mutex};
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema, TimeUnit};
// Local Imports
use super::capacity::Reservation;
use crate::{Error, MemorySize, Result};

/* === Constants === */

pub(crate) const MAX_DEPTH: usize = 32;
pub(crate) const MAX_FIELDS: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct RetainedMemory {
    // Arrow Allocation Requires Unwind-Safe Shared Ownership; The Mutex Owner
    // Keeps The One Storage Reservation Type Behind That Contract.
    owner: Arc<Mutex<Reservation>>,
}

#[derive(Clone, Debug)]
pub struct ArrowSchema {
    schema: Arc<Schema>,
    ipc: Arc<Vec<u8>>,
    retention: Option<Retained>,
}

#[derive(Clone, Debug)]
pub struct ArrowBatch {
    batch: RecordBatch,
    ipc: Arc<Vec<u8>>,
    resident: usize,
    retention: Option<Retained>,
}

#[derive(Clone)]
struct Retained(RetainedMemory);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl RetainedMemory {
    pub(super) fn new(owner: Arc<Mutex<Reservation>>) -> Self {
        Self { owner }
    }

    /// Total admitted bytes, including any private owner allowance.
    pub fn bytes(&self) -> usize {
        self.owner.lock().unwrap().bytes()
    }

    /// Admits an absolute total high-water mark; this never shrinks the claim.
    /// Concurrent incremental allocators must use `try_grow_by`.
    #[tracing::instrument(name = "storage.arrow.memory.try_grow_to", level = "trace", skip_all, fields(bytes = bytes, outcome = "cancelled"))]
    pub fn try_grow_to(&self, bytes: usize) -> Result<()> {
        let result: Result<()> = self.try_grow(bytes).map_err(crate::capacity::Refusal::into_error);
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(crate) fn try_grow(&self, bytes: usize) -> std::result::Result<(), crate::capacity::Refusal> {
        let mut owner = self.owner.lock().unwrap();
        let additional = bytes.saturating_sub(owner.bytes());
        owner.try_grow(additional)
    }

    /// Atomically admits an additional allocation peak without waiting.
    ///
    /// Refusal leaves the claim unchanged. Use this for independent increments
    /// through aliases; `bytes() + try_grow_to()` is not an atomic increment.
    #[tracing::instrument(name = "storage.arrow.memory.try_grow_by", level = "trace", skip_all, fields(additional = additional, outcome = "cancelled"))]
    pub fn try_grow_by(&self, additional: usize) -> Result<()> {
        let result: Result<()> = self.owner.lock().unwrap().try_grow(additional).map_err(crate::capacity::Refusal::into_error);
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(crate) async fn grow_to(&self, bytes: usize) -> std::result::Result<(), crate::capacity::Refusal> {
        Reservation::grow_shared(&self.owner, bytes).await
    }

    pub(crate) fn shrink_to(&self, bytes: usize) -> Result<()> {
        let mut held = self.owner.lock().unwrap();
        if bytes > held.bytes() {
            return Err(invalid("retained memory cannot grow after allocation"));
        }
        held.shrink_to(bytes);
        Ok(())
    }
}

impl std::fmt::Debug for RetainedMemory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RetainedMemory")
    }
}

impl std::fmt::Debug for Retained {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _owner = &self.0;
        formatter.write_str("RetainedMemory")
    }
}

impl ArrowSchema {
    #[tracing::instrument(name = "storage.arrow.schema.try_new", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn try_new(schema: Arc<Schema>) -> Result<Self> {
        let result: Result<Self> = (|| {
            validate_schema(&schema)?;
            let ipc = Arc::new(ipc::encode(&schema, None)?);
            Ok(Self { schema, ipc, retention: None })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(crate) fn retain(mut self, reservation: RetainedMemory) -> Self {
        self.retention = Some(Retained(reservation));
        self
    }

    pub fn as_schema(&self) -> &Schema {
        &self.schema
    }

    pub(crate) fn ipc_capacity(&self) -> usize {
        self.ipc.capacity()
    }

    pub fn ipc_bytes(&self) -> &[u8] {
        &self.ipc
    }

    #[tracing::instrument(name = "storage.arrow.schema.from_ipc", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn from_ipc(bytes: Vec<u8>) -> Result<Self> {
        let result: Result<Self> = (|| {
            ipc::preflight(&bytes, false)?;
            let (schema, _) = ipc::decode(&bytes, false)?;
            Ok(Self { schema, ipc: Arc::new(bytes), retention: None })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }
}

impl ArrowBatch {
    #[tracing::instrument(name = "storage.arrow.batch.try_new", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn try_new(batch: RecordBatch) -> Result<Self> {
        let result: Result<Self> = (|| {
            validate_schema(batch.schema().as_ref())?;
            let encoded = ipc::encode(batch.schema().as_ref(), Some(&batch))?;
            let resident = retention::resident_bytes(&batch)?;
            Ok(Self { batch, ipc: Arc::new(encoded), resident, retention: None })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(crate) fn retain(mut self, reservation: RetainedMemory) -> Result<Self> {
        let owner =
            Arc::new(retention::Owner { _reservation: reservation.clone(), _encoded: Arc::clone(&self.ipc), _schema: self.batch.schema() });
        self.batch = retention::attach(&self.batch, owner)?;
        self.retention = Some(Retained(reservation));
        Ok(self)
    }

    /// Projects an admitted batch while preserving its original shared claim.
    ///
    /// `additional_peak` bounds new output allocations and conversion scratch.
    /// Private output-owner storage is admitted separately, before the callback.
    /// Immutable Arrow buffer clones and slices may share the original claim.
    /// The callback must transfer payloads only into its successful value; errors
    /// and external captures must not retain them. Nested owners retain their guards.
    /// Unretained batches from `try_new` or `from_ipc` refuse without conversion.
    #[tracing::instrument(name = "storage.arrow.batch.try_map", level = "trace", skip_all, fields(additional_peak = additional_peak, outcome = "cancelled"))]
    pub fn try_map<U, E>(
        self, additional_peak: usize, map: impl FnOnce(RecordBatch) -> std::result::Result<U, E>,
    ) -> std::result::Result<crate::ReadBatch<U>, E>
    where
        E: From<Error>,
    {
        let result: std::result::Result<crate::ReadBatch<U>, E> = (|| {
            let memory = self.retention.as_ref().ok_or_else(|| E::from(invalid("projection requires an admitted batch")))?.0.clone();
            let additional = additional_peak
                .checked_add(crate::ReadBatch::<U>::owner_bytes().map_err(E::from)?)
                .ok_or_else(|| E::from(Error::ResourceExhausted { resource: "Arrow projection memory" }))?;
            memory.try_grow_by(additional).map_err(E::from)?;
            let Self { batch, .. } = self;
            let value = map(batch)?;
            Ok(crate::ReadBatch::retained(value, memory))
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn as_record_batch(&self) -> &RecordBatch {
        &self.batch
    }

    pub(crate) fn ipc_capacity(&self) -> usize {
        self.ipc.capacity()
    }

    pub fn ipc_bytes(&self) -> &[u8] {
        &self.ipc
    }

    pub(crate) fn encoding_memory_size(&self) -> Result<usize> {
        self.ipc
            .capacity()
            .checked_add(retention::owner_bytes(&self.batch)?)
            .ok_or_else(|| invalid("Arrow encoding ownership size overflow"))
    }

    #[tracing::instrument(name = "storage.arrow.batch.from_ipc", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn from_ipc(bytes: Vec<u8>) -> Result<Self> {
        let result: Result<Self> = (|| {
            ipc::preflight(&bytes, true)?;
            let (_, batch) = ipc::decode(&bytes, true)?;
            let batch = batch.ok_or_else(|| invalid("missing Arrow record batch"))?;
            let resident = retention::resident_bytes(&batch)?;
            Ok(Self { batch, ipc: Arc::new(bytes), resident, retention: None })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn validate_schema(&self, schema: &Schema) -> Result<()> {
        if self.batch.schema().as_ref() != schema {
            return Err(invalid("batch schema differs from announced schema"));
        }
        Ok(())
    }
}

impl MemorySize for ArrowSchema {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(schema_memory(&self.schema)).saturating_add(self.ipc.capacity())
    }
}

impl From<ArrowSchema> for harmos_domain::model::storage::ArrowIpc {
    fn from(schema: ArrowSchema) -> Self {
        Self::from(schema.ipc)
    }
}

impl From<ArrowBatch> for harmos_domain::model::storage::ArrowIpc {
    fn from(batch: ArrowBatch) -> Self {
        let ArrowBatch { batch, ipc, retention, .. } = batch;
        drop(batch);
        let encoded = Self::from(ipc);
        drop(retention);
        encoded
    }
}

impl MemorySize for ArrowBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.resident).saturating_add(self.ipc.capacity())
    }
}

/* === Public Functions === */

pub fn validate_schema(schema: &Schema) -> Result<()> {
    let mut fields = 0;
    for field in schema.fields() {
        if schema.fields().iter().filter(|other| other.name() == field.name()).count() != 1 {
            return Err(invalid("duplicate Arrow field name"));
        }
        validate_field(field, 0, &mut fields)?;
    }
    Ok(())
}

#[tracing::instrument(name = "storage.arrow.decoding_memory", level = "trace", skip_all, fields(bytes = bytes.len(), batch = batch, outcome = "cancelled"))]
pub fn decoding_memory(bytes: &[u8], batch: bool) -> Result<usize> {
    let result: Result<usize> = ipc::preflight(bytes, batch);
    tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
    result
}

/* === Private Functions === */

fn validate_field(field: &Field, depth: usize, count: &mut usize) -> Result<()> {
    *count = count.checked_add(1).ok_or_else(|| invalid("Arrow field count overflow"))?;
    if depth > MAX_DEPTH || *count > MAX_FIELDS {
        return Err(invalid("Arrow schema exceeds structural bounds"));
    }
    match field.data_type() {
        DataType::Null
        | DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float32
        | DataType::Float64
        | DataType::Utf8
        | DataType::Binary => Ok(()),
        DataType::Timestamp(TimeUnit::Nanosecond, zone) if zone.as_deref() == Some("UTC") => Ok(()),
        DataType::List(child) => validate_field(child, depth + 1, count),
        DataType::Struct(children) => {
            for child in children {
                if children.iter().filter(|other| other.name() == child.name()).count() != 1 {
                    return Err(invalid("duplicate Arrow child field name"));
                }
                validate_field(child, depth + 1, count)?;
            }
            Ok(())
        }
        _ => Err(invalid("unsupported Arrow data type")),
    }
}

pub(crate) fn schema_memory(schema: &Schema) -> usize {
    std::mem::size_of::<Schema>()
        .saturating_add(schema.fields().iter().map(|field| field.size()).fold(0usize, usize::saturating_add))
        .saturating_add(
            schema
                .metadata()
                .iter()
                .map(|(key, value)| key.capacity().saturating_add(value.capacity()).saturating_add(64))
                .fold(0usize, usize::saturating_add),
        )
}

pub(crate) fn resident_bytes(batch: &RecordBatch) -> Result<usize> {
    retention::resident_bytes(batch)
}

pub(crate) fn new_resident_bytes(batch: &RecordBatch, input: &[&RecordBatch]) -> Result<usize> {
    retention::new_resident_bytes(batch, input)
}

pub(crate) fn retain_native(batch: RecordBatch, reservation: RetainedMemory) -> Result<RecordBatch> {
    let owner = Arc::new(retention::Owner { _reservation: reservation, _encoded: Arc::new(Vec::new()), _schema: batch.schema() });
    retention::attach(&batch, owner)
}

pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage Arrow", reason }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{ArrowBatch, ArrowSchema, decoding_memory};
    use crate::MemorySize;
    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![Field::new("key", DataType::Int64, false), Field::new("row", DataType::Int64, false)]));
        RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![2, 1, 1, 2])), Arc::new(Int64Array::from(vec![0, 1, 2, 3]))])
            .unwrap()
    }

    #[test]
    fn ipc_codec_roundtrips() {
        // Native Buffers And Self-Contained Wire Values Agree.
        let original = batch();
        let encoded = ArrowBatch::try_new(original.clone()).unwrap();
        let bound = decoding_memory(encoded.ipc_bytes(), true).unwrap();
        let bytes = encoded.ipc_bytes().to_vec();
        let allocation = bytes.as_ptr();
        let decoded = ArrowBatch::from_ipc(bytes).unwrap();
        assert_eq!(decoded.ipc_bytes().as_ptr(), allocation);
        assert_eq!(decoded.as_record_batch(), &original);
        assert!(bound >= decoded.memory_size());
        let header = ArrowSchema::try_new(original.schema()).unwrap();
        let decoded_header = ArrowSchema::from_ipc(header.ipc_bytes().to_vec()).unwrap();
        decoded.validate_schema(decoded_header.as_schema()).unwrap();
        let output = harmos_domain::model::storage::ArrowIpc::from(decoded);
        assert_eq!(output.bytes().as_ptr(), allocation);
        assert_eq!(output.bytes(), encoded.ipc_bytes());
        let shared = encoded.clone();
        let output = harmos_domain::model::storage::ArrowIpc::from(shared);
        assert_eq!(output.bytes(), encoded.ipc_bytes());
        assert_ne!(output.bytes().as_ptr(), encoded.ipc_bytes().as_ptr());
        assert!(ArrowSchema::from_ipc(encoded.ipc_bytes().to_vec()).is_err());
        assert!(ArrowBatch::from_ipc(header.ipc_bytes().to_vec()).is_err());

        // Malformed Framing Is Refused Before Arrow Allocates Arrays.
        for length in [0, 1, 4, encoded.ipc_bytes().len() - 1] {
            assert!(decoding_memory(&encoded.ipc_bytes()[..length], true).is_err());
        }
        let mut trailing = encoded.ipc_bytes().to_vec();
        trailing.extend_from_slice(&[0, 0, 0, 0]);
        assert!(decoding_memory(&trailing, true).is_err());

        // Unsupported And Ambiguous Schemas Are Refused.
        let unsupported = Schema::new(vec![Field::new("value", DataType::LargeUtf8, false)]);
        assert!(ArrowSchema::try_new(Arc::new(unsupported)).is_err());
        let duplicate = Schema::new(vec![Field::new("x", DataType::Int64, false), Field::new("x", DataType::Int64, false)]);
        assert!(ArrowSchema::try_new(Arc::new(duplicate)).is_err());
    }
    fn admitted(bytes: usize) -> (crate::StorageManager, ArrowBatch) {
        use crate::capacity::{Held, Pool, Reservation};
        use crate::test_backend::{Data, Recovery, history, storage_settings};
        let storage = crate::StorageManager::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            storage_settings(),
        )
        .unwrap();
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.ready = true;
            state.status.running = true;
        }
        let memory = Reservation::try_new(&storage.inner, Pool::Read, &Held::default(), bytes).unwrap().retain();
        let batch = ArrowBatch::try_new(batch()).unwrap().retain(memory).unwrap();
        (storage, batch)
    }

    #[test]
    fn projection_aliases_retain() {
        let (storage, batch) = admitted(16_384);
        let alias = batch.clone();
        let slice = batch.as_record_batch().slice(1, 2);
        let pointer = batch.as_record_batch().column(0).as_any().downcast_ref::<Int64Array>().unwrap().values().as_ptr();
        let projected = batch.try_map(0, Ok::<_, crate::Error>).unwrap();
        assert_eq!(projected.column(0).as_any().downcast_ref::<Int64Array>().unwrap().values().as_ptr(), pointer);
        drop(projected);
        drop(alias);
        assert!(storage.status().read_bytes > 0);
        assert_eq!(slice.column(0).as_any().downcast_ref::<Int64Array>().unwrap().value(0), 1);
        drop(slice);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_dto_retains() {
        let (storage, batch) = admitted(16_384);
        let projected = batch
            .try_map(4 * std::mem::size_of::<i64>(), |batch| {
                Ok::<_, crate::Error>(batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap().values().to_vec())
            })
            .unwrap();
        assert_eq!(projected.as_slice(), &[2, 1, 1, 2]);
        assert!(storage.status().read_bytes > 16_384);
        drop(projected);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn unretained_projection_refuses() {
        let encoded = ArrowBatch::try_new(batch()).unwrap();
        let decoded = ArrowBatch::from_ipc(encoded.ipc_bytes().to_vec()).unwrap();
        for batch in [encoded, decoded] {
            let called = std::cell::Cell::new(false);
            let result = batch.try_map(0, |batch| {
                called.set(true);
                Ok::<_, crate::Error>(batch)
            });
            assert!(result.is_err());
            assert!(!called.get());
        }
    }

    #[test]
    fn projection_budget_refuses() {
        let bytes = crate::test_backend::storage_settings().budget.read_memory_bytes.get();
        let (storage, batch) = admitted(bytes);
        let called = std::cell::Cell::new(false);
        let result = batch.try_map(0, |batch| {
            called.set(true);
            Ok::<_, crate::Error>(batch)
        });
        assert!(matches!(result, Err(crate::Error::ResourceExhausted { .. })));
        assert!(!called.get());
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_failure_retains() {
        let (storage, batch) = admitted(16_384);
        let alias = batch.clone();
        let result = batch.try_map(64, |_| -> crate::Result<()> {
            Err(crate::Error::InvalidInput { field: "projection", reason: "injected conversion failure" })
        });
        assert!(result.is_err());
        assert!(storage.status().read_bytes > 16_384);
        drop(alias);
        assert_eq!(storage.status().read_bytes, 0);
    }

    #[test]
    fn projection_alias_growth() {
        let (storage, batch) = admitted(16_384);
        let values = std::thread::scope(|scope| {
            let mut tasks = Vec::new();
            for _ in 0..2 {
                let batch = batch.clone();
                tasks.push(scope.spawn(move || batch.try_map(0, |batch| Ok::<_, crate::Error>(batch.num_rows())).unwrap()));
            }
            tasks.into_iter().map(|task| task.join().unwrap()).collect::<Vec<_>>()
        });
        let scaffold = crate::ReadBatch::<usize>::owner_bytes().unwrap();
        assert_eq!(storage.status().read_bytes, 16_384 + 2 * scaffold);
        drop(batch);
        assert!(values.iter().all(|value| **value == 4));
        drop(values);
        assert_eq!(storage.status().read_bytes, 0);
    }
}
