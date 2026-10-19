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

pub use super::writes::convert::encode_payloads;

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

/// An admitted reservation retained until the last Arrow allocation is released.
/// Admission and resizing are storage-internal; consumers cannot release a live
/// result's charge while retaining its native zero-copy arrays.
#[derive(Clone)]
pub struct RetainedMemory {
    // Arrow Allocation Requires Unwind-Safe Shared Ownership; The Mutex Owner
    // Keeps The One Storage Reservation Type Behind That Contract.
    owner: Arc<Mutex<Reservation>>,
}

/// Native Arrow schema with an explicitly encoded, immutable IPC representation.
#[derive(Clone, Debug)]
pub struct ArrowSchema {
    schema: Arc<Schema>,
    ipc: Arc<Vec<u8>>,
    retention: Option<Retained>,
}

/// Native Arrow data. Zero-copy clones and slices retain admitted buffer owners.
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

    /// Shrink the charge after construction scratch is released. Growing a
    /// retained charge is refused: allocations must be admitted before they exist.
    pub(crate) fn shrink_to(&self, bytes: usize) -> Result<()> {
        let mut held = self.owner.lock().unwrap();
        if bytes > held.bytes() {
            return Err(invalid("retained memory cannot grow after allocation"));
        }
        held.shrink_to(bytes);
        Ok(())
    }
}

impl std::fmt::Debug for Retained {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _owner = &self.0;
        formatter.write_str("RetainedMemory")
    }
}

impl ArrowSchema {
    pub fn try_new(schema: Arc<Schema>) -> Result<Self> {
        validate_schema(&schema)?;
        let ipc = Arc::new(ipc::encode(&schema, None)?);
        Ok(Self { schema, ipc, retention: None })
    }

    pub(crate) fn retain(mut self, reservation: RetainedMemory) -> Self {
        self.retention = Some(Retained(reservation));
        self
    }

    pub fn as_schema(&self) -> &Schema {
        &self.schema
    }

    pub fn ipc_bytes(&self) -> &[u8] {
        &self.ipc
    }

    pub fn from_ipc(bytes: &[u8]) -> Result<Self> {
        ipc::preflight(bytes, false)?;
        let (schema, _) = ipc::decode(bytes, false)?;
        Ok(Self { schema, ipc: Arc::new(bytes.to_vec()), retention: None })
    }
}

impl ArrowBatch {
    /// Construct consumer-owned data. Storage admission attaches ownership with `retain`.
    pub fn try_new(batch: RecordBatch) -> Result<Self> {
        validate_schema(batch.schema().as_ref())?;
        let encoded = ipc::encode(batch.schema().as_ref(), Some(&batch))?;
        let resident = retention::resident_bytes(&batch)?;
        Ok(Self { batch, ipc: Arc::new(encoded), resident, retention: None })
    }

    /// Attach a reservation covering the native buffers, schema, and cached encoding.
    /// Every returned zero-copy array retains that reservation in its buffer owner.
    pub(crate) fn retain(mut self, reservation: RetainedMemory) -> Result<Self> {
        let owner =
            Arc::new(retention::Owner { _reservation: reservation.clone(), _encoded: Arc::clone(&self.ipc), _schema: self.batch.schema() });
        self.batch = retention::attach(&self.batch, owner)?;
        self.retention = Some(Retained(reservation));
        Ok(self)
    }

    pub fn as_record_batch(&self) -> &RecordBatch {
        &self.batch
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

    pub fn from_ipc(bytes: &[u8]) -> Result<Self> {
        ipc::preflight(bytes, true)?;
        let (_, batch) = ipc::decode(bytes, true)?;
        let batch = batch.ok_or_else(|| invalid("missing Arrow record batch"))?;
        let resident = retention::resident_bytes(&batch)?;
        Ok(Self { batch, ipc: Arc::new(bytes.to_vec()), resident, retention: None })
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

impl From<&ArrowBatch> for harmos_domain::model::storage::ArrowIpc {
    fn from(batch: &ArrowBatch) -> Self {
        Self::new(batch.ipc_bytes().to_vec())
    }
}

impl From<ArrowBatch> for harmos_domain::model::storage::ArrowIpc {
    fn from(batch: ArrowBatch) -> Self {
        Self::from(&batch)
    }
}

impl MemorySize for ArrowBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.resident).saturating_add(self.ipc.capacity())
    }
}

/* === Public Functions === */

/// Reject unsupported Arrow representations before output or decoding.
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

/// Conservative bound including wire staging, Arrow decoding, and retained data.
pub fn decoding_memory(bytes: &[u8], batch: bool) -> Result<usize> {
    ipc::preflight(bytes, batch)
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
