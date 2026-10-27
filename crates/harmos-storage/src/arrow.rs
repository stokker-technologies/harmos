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
use std::sync::Arc;
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema, TimeUnit};
// Local Imports
use crate::{Error, MemorySize, Result};

/* === Constants === */

pub(crate) const MAX_DEPTH: usize = 32;
pub(crate) const MAX_FIELDS: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ArrowSchema {
    schema: Arc<Schema>,
    ipc: Arc<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ArrowBatch {
    batch: RecordBatch,
    ipc: Arc<Vec<u8>>,
    resident: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ArrowSchema {
    #[tracing::instrument(name = "storage.arrow.schema.try_new", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn try_new(schema: Arc<Schema>) -> Result<Self> {
        let result: Result<Self> = (|| {
            validate_schema(&schema)?;
            let ipc = Arc::new(ipc::encode(&schema, None)?);
            Ok(Self { schema, ipc })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn as_schema(&self) -> &Schema {
        &self.schema
    }

    pub fn ipc_capacity(&self) -> usize {
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
            Ok(Self { schema, ipc: Arc::new(bytes) })
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
            Ok(Self { batch, ipc: Arc::new(encoded), resident })
        })();
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn try_map<U, E>(self, map: impl FnOnce(RecordBatch) -> std::result::Result<U, E>) -> std::result::Result<crate::ReadBatch<U>, E> {
        Ok(crate::ReadBatch::new(map(self.batch)?))
    }

    pub fn as_record_batch(&self) -> &RecordBatch {
        &self.batch
    }

    pub fn ipc_capacity(&self) -> usize {
        self.ipc.capacity()
    }

    pub fn ipc_bytes(&self) -> &[u8] {
        &self.ipc
    }

    #[tracing::instrument(name = "storage.arrow.batch.from_ipc", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub fn from_ipc(bytes: Vec<u8>) -> Result<Self> {
        let result: Result<Self> = (|| {
            ipc::preflight(&bytes, true)?;
            let (_, batch) = ipc::decode(&bytes, true)?;
            let batch = batch.ok_or_else(|| invalid("missing Arrow record batch"))?;
            let resident = retention::resident_bytes(&batch)?;
            Ok(Self { batch, ipc: Arc::new(bytes), resident })
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
        let ArrowBatch { batch, ipc, .. } = batch;
        drop(batch);
        Self::from(ipc)
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

pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage Arrow", reason }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{ArrowBatch, ArrowSchema, ipc};
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
        ipc::preflight(encoded.ipc_bytes(), true).unwrap();
        let bytes = encoded.ipc_bytes().to_vec();
        let allocation = bytes.as_ptr();
        let decoded = ArrowBatch::from_ipc(bytes).unwrap();
        assert_eq!(decoded.ipc_bytes().as_ptr(), allocation);
        assert_eq!(decoded.as_record_batch(), &original);
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
            assert!(ipc::preflight(&encoded.ipc_bytes()[..length], true).is_err());
        }
        let mut trailing = encoded.ipc_bytes().to_vec();
        trailing.extend_from_slice(&[0, 0, 0, 0]);
        assert!(ipc::preflight(&trailing, true).is_err());

        // Unsupported And Ambiguous Schemas Are Refused.
        let unsupported = Schema::new(vec![Field::new("value", DataType::LargeUtf8, false)]);
        assert!(ArrowSchema::try_new(Arc::new(unsupported)).is_err());
        let duplicate = Schema::new(vec![Field::new("x", DataType::Int64, false), Field::new("x", DataType::Int64, false)]);
        assert!(ArrowSchema::try_new(Arc::new(duplicate)).is_err());
    }
}
