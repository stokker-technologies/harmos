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

use crate::ObjectEntry;
use crate::StoredRecord;
use crate::StoredRow;
use arrow_schema::Field;
use serde_json::Error as JsonError;
use std::cmp::Ordering as CmpOrdering;

// Standard Library Imports
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Third Party Imports
use arrow_array::{Array, RecordBatch, UInt64Array};
use arrow_schema::DataType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::scalar::ScalarValue;
use super::transform::{
    FieldPath, ProjectExecution,
    project::{Project, Projection},
};
use crate::{Error, MemorySize, Result, SchemaKey};
use harmos_domain::model::storage::StorageSchema;
use harmos_domain::model::storage::{BackendSnapshot, ReadLimits};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancellationState>);

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    parent: Option<Cancellation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSource {
    pub kind: ColumnKind,
    pub schema: SchemaKey,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnPredicate {
    pub column: String,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeBounds {
    pub column: String,
    pub lower_ns: i64,
    pub upper_ns: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrowSeek {
    pub snapshot: Vec<u8>,
    pub plan: [u8; 32],
    pub position: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrowScan {
    pub source: ColumnSource,
    pub columns: Vec<String>,
    pub predicate: Option<ColumnPredicate>,
    pub time: Option<TimeBounds>,
    pub start: u64,
    pub after: Option<ArrowSeek>,
    pub limits: ReadLimits,
    pub work: ReadLimits,
}

pub struct ScanEvidence {
    pub complete: bool,
    pub bytes: usize,
    pub columns: Vec<String>,
    pub payload_rows: usize,
}

#[derive(Debug)]
pub struct ArrowPage {
    pub batch: RecordBatch,
    pub next: Option<ArrowSeek>,
    pub complete: bool,
    pub scanned: usize,
    pub bytes_read: usize,
    pub columns_read: Vec<String>,
    pub payload_rows: usize,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnKind {
    Objects,
    Records,
    Stream,
}

/* === Traits === */

/* === Implementations === */

impl MemorySize for ColumnSource {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.schema.memory_size())
    }
}

impl MemorySize for ColumnPredicate {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.column.memory_size())
    }
}

impl MemorySize for TimeBounds {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.column.memory_size())
    }
}

impl MemorySize for ArrowPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.batch.get_array_memory_size()).saturating_add(self.columns_read.memory_size())
    }
}

impl Cancellation {
    pub fn child(&self) -> Self {
        Self(Arc::new(CancellationState {
            cancelled: AtomicBool::new(false),
            parent: Some(self.clone()),
        }))
    }

    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }

    pub fn check(&self) -> Result<()> {
        let mut current = Some(self);
        while let Some(cancel) = current {
            if cancel.0.cancelled.load(Ordering::Acquire) {
                return Err(Error::Closing { domain: "cancelled storage query" });
            }
            current = cancel.0.parent.as_ref();
        }
        Ok(())
    }
}

impl ArrowScan {
    pub fn identity(&self) -> Result<[u8; 32]> {
        let encoded = serde_json::to_vec(&(&self.source, &self.columns, &self.predicate, &self.time, self.start)).map_err(encoding)?;
        Ok(Sha256::digest(encoded).into())
    }

    pub fn position(&self, snapshot: &BackendSnapshot) -> Result<u64> {
        self.validate()?;
        if let Some(seek) = &self.after {
            if seek.snapshot != snapshot.token
                || seek.plan != self.identity()?
                || seek.position < self.start
                || seek.position > snapshot.at.sequence
            {
                return Err(invalid("continuation belongs to a different snapshot or plan"));
            }
            return Ok(seek.position);
        }
        Ok(self.start)
    }

    pub fn validate(&self) -> Result<()> {
        if self.columns.is_empty() || self.columns.len() > crate::arrow::MAX_FIELDS || self.columns.iter().any(String::is_empty) {
            return Err(invalid("scan requires nonempty bounded column names"));
        }
        if self.columns.iter().collect::<BTreeSet<_>>().len() != self.columns.len() {
            return Err(invalid("scan columns must be unique"));
        }
        if let Some(predicate) = &self.predicate {
            if predicate.column.is_empty()
                || predicate.lower.is_some_and(|value| !value.is_finite())
                || predicate.upper.is_some_and(|value| !value.is_finite())
            {
                return Err(invalid("predicate requires a column and finite bounds"));
            }
            if matches!((predicate.lower, predicate.upper), (Some(lower), Some(upper)) if lower >= upper) {
                return Err(invalid("predicate requires increasing half-open bounds"));
            }
        }
        if self.time.as_ref().is_some_and(|time| time.column.is_empty() || time.lower_ns >= time.upper_ns) {
            return Err(invalid("time window requires a column and increasing half-open bounds"));
        }
        Ok(())
    }

    pub fn required_columns(&self) -> Vec<String> {
        let mut names = self.columns.clone();
        names.push("_storage.position".into());
        if let Some(predicate) = &self.predicate {
            names.push(predicate.column.clone());
        }
        if let Some(time) = &self.time {
            names.push(time.column.clone());
        }
        names.sort();
        names.dedup();
        names
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub fn flatten(batch: &RecordBatch) -> Result<RecordBatch> {
    let mut paths = Vec::new();
    for field in batch.schema().fields() {
        leaves(field, String::new(), &mut paths);
    }
    let project = Project {
        fields: paths
            .into_iter()
            .map(|name| Projection {
                field: FieldPath::from(name.as_str()),
                name,
            })
            .collect(),
    };
    project.apply(batch, &mut || Ok(()))
}

fn leaves(field: &Field, parent: String, paths: &mut Vec<String>) {
    let name = if parent.is_empty() { field.name().clone() } else { format!("{parent}.{}", field.name()) };
    if let DataType::Struct(fields) = field.data_type() {
        for field in fields {
            leaves(field, name.clone(), paths);
        }
    } else {
        paths.push(name);
    }
}

pub fn empty(schema: &StorageSchema, kind: ColumnKind) -> Result<RecordBatch> {
    let batch = match kind {
        ColumnKind::Objects => crate::source::objects(schema, &[])?,
        ColumnKind::Records => crate::source::records(schema, std::iter::empty())?,
        ColumnKind::Stream => crate::source::stream(schema, &[])?,
    };
    flatten(&batch)
}

pub fn records(schema: &StorageSchema, rows: &[StoredRecord]) -> Result<RecordBatch> {
    flatten(&crate::source::records(schema, rows.iter())?)
}
pub fn streams(schema: &StorageSchema, rows: &[StoredRow]) -> Result<RecordBatch> {
    flatten(&crate::source::stream(schema, rows)?)
}
pub fn objects(schema: &StorageSchema, rows: &[ObjectEntry]) -> Result<RecordBatch> {
    flatten(&crate::source::objects(schema, rows)?)
}

pub fn finish_page(
    snapshot: &BackendSnapshot, read: &ArrowScan, batch: &RecordBatch, evidence: ScanEvidence, cancel: &Cancellation,
) -> Result<ArrowPage> {
    let ScanEvidence {
        complete: source_complete,
        bytes: bytes_read,
        columns: columns_read,
        payload_rows,
    } = evidence;
    cancel.check()?;
    let after = read.position(snapshot)?;
    let positions = batch
        .column_by_name("_storage.position")
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .ok_or_else(|| invalid("source positions must be UInt64"))?;
    for name in read.required_columns() {
        if batch.column_by_name(&name).is_none() {
            return Err(invalid("requested column is unavailable"));
        }
    }
    if let Some(predicate) = &read.predicate {
        let array = batch.column_by_name(&predicate.column).expect("validated column");
        if !matches!(
            array.data_type(),
            DataType::Float32
                | DataType::Float64
                | DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        ) {
            return Err(invalid("predicate column must be numeric"));
        }
    }
    if let Some(time) = &read.time {
        let array = batch.column_by_name(&time.column).expect("validated column");
        if !matches!(array.data_type(), DataType::Int64 | DataType::Timestamp(arrow_schema::TimeUnit::Nanosecond, _)) {
            return Err(invalid("time column must be Int64 or nanosecond timestamp"));
        }
    }
    let output_indices = read
        .columns
        .iter()
        .map(|name| batch.schema().index_of(name).map_err(|_| invalid("requested column is unavailable")))
        .collect::<Result<Vec<_>>>()?;
    let output = batch.project(&output_indices).map_err(|_| invalid("column projection failed"))?;
    let mut indices = Vec::new();
    let mut last = after;
    let mut scanned = 0;
    let mut consumed = true;
    for row in 0..batch.num_rows() {
        if row % 256 == 0 {
            cancel.check()?;
        }
        let position = positions.value(row);
        if position <= after {
            continue;
        }
        if position > snapshot.at.sequence {
            break;
        }
        if scanned == read.work.rows.get() || indices.len() == read.limits.rows.get() {
            consumed = false;
            break;
        }
        scanned += 1;
        let matches = matches_row(batch, row, read)?;
        if matches {
            indices.push(row as u64);
        }
        last = position;
    }
    let mut batch = take(&output, &indices)?;
    if page_bytes(&batch)? > read.limits.bytes.get() {
        let mut low = 0;
        let mut high = indices.len();
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            let candidate = take(&output, &indices[..middle])?;
            if page_bytes(&candidate)? <= read.limits.bytes.get() {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        if low == 0 {
            return Err(exhausted());
        }
        indices.truncate(low);
        last = positions.value(*indices.last().expect("nonempty selection") as usize);
        scanned = positions.values().iter().filter(|position| **position > after && **position <= last).count();
        consumed = false;
        batch = take(&output, &indices)?;
    }
    let complete = consumed && (source_complete || last >= snapshot.at.sequence);
    let next = (!complete).then(|| ArrowSeek {
        snapshot: snapshot.token.clone(),
        plan: read.identity().expect("validated plan"),
        position: last,
    });
    if !complete && last == after && batch.num_rows() != 0 {
        return Err(invalid("backend scan did not advance"));
    }
    Ok(ArrowPage {
        batch,
        next,
        complete,
        scanned,
        bytes_read,
        columns_read,
        payload_rows,
    })
}

pub fn page_bytes(batch: &RecordBatch) -> Result<usize> {
    let ipc = crate::arrow::ArrowBatch::try_new(batch.clone())?;
    Ok(batch.get_array_memory_size().max(ipc.ipc_bytes().len()))
}

fn matches_row(batch: &RecordBatch, row: usize, read: &ArrowScan) -> Result<bool> {
    if let Some(time) = &read.time {
        let array = batch.column_by_name(&time.column).ok_or_else(|| invalid("time column is unavailable"))?;
        let value = super::transform::scalar(array.as_ref(), row)?;
        let value = match value {
            ScalarValue::Signed(value) => i128::from(value),
            ScalarValue::Datetime(value) => value,
            ScalarValue::Null => return Ok(false),
            _ => return Err(invalid("time column must be Int64 or nanosecond timestamp")),
        };
        if value < i128::from(time.lower_ns) || value >= i128::from(time.upper_ns) {
            return Ok(false);
        }
    }
    if let Some(predicate) = &read.predicate {
        let array = batch.column_by_name(&predicate.column).ok_or_else(|| invalid("predicate column is unavailable"))?;
        let value = super::transform::scalar(array.as_ref(), row)?;
        if matches!(value, ScalarValue::Null | ScalarValue::Missing) || matches!(value, ScalarValue::Float(value) if !value.is_finite()) {
            return Ok(false);
        }
        if !matches!(value, ScalarValue::Float(_) | ScalarValue::Signed(_) | ScalarValue::Unsigned(_)) {
            return Err(invalid("predicate column must be numeric"));
        }
        if let Some(lower) = predicate.lower
            && compare_bound(&value, lower)?.is_lt()
        {
            return Ok(false);
        }

        if let Some(upper) = predicate.upper
            && !compare_bound(&value, upper)?.is_lt()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compare_bound(value: &ScalarValue, bound: f64) -> Result<CmpOrdering> {
    if !bound.is_finite() {
        return Err(invalid("ListColumnRows predicate bound must be finite"));
    }
    let integer = match value {
        ScalarValue::Signed(value) => i128::from(*value),
        ScalarValue::Unsigned(value) => i128::from(*value),
        ScalarValue::Float(value) => {
            return value.partial_cmp(&bound).ok_or_else(|| invalid("ListColumnRows predicate value must be finite"));
        }
        _ => return Err(invalid("ListColumnRows predicate column must be numeric")),
    };
    // These Powers Of Two Are Exact F64 Endpoints Around Every Supported Integer.
    if bound >= 18_446_744_073_709_551_616.0 {
        return Ok(CmpOrdering::Less);
    }
    if bound < i64::MIN as f64 {
        return Ok(CmpOrdering::Greater);
    }
    let order = integer.cmp(&(bound as i128));
    if order != CmpOrdering::Equal {
        return Ok(order);
    }
    Ok(if bound.fract() > 0.0 {
        CmpOrdering::Less
    } else if bound.fract() < 0.0 {
        CmpOrdering::Greater
    } else {
        CmpOrdering::Equal
    })
}

pub fn take(batch: &RecordBatch, indices: &[u64]) -> Result<RecordBatch> {
    let indices = UInt64Array::from(indices.to_vec());
    let columns = batch
        .columns()
        .iter()
        .map(|column| arrow_select::take::take(column.as_ref(), &indices, None).map_err(|_| invalid("Arrow row selection failed")))
        .collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new_with_options(batch.schema(), columns, &arrow_array::RecordBatchOptions::new().with_row_count(Some(indices.len())))
        .map_err(|_| invalid("invalid selected Arrow batch"))
}

pub fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage columns", reason }
}
pub fn exhausted() -> Error {
    Error::ResourceExhausted {
        resource: "storage column page or work budget",
    }
}
fn encoding(source: JsonError) -> Error {
    Error::backend("encode column plan", crate::BackendFailure::Permanent, source)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use CmpOrdering::{Equal, Greater, Less};

    #[test]
    fn numeric_column_bounds_preserve_integer_endpoints() {
        for (value, bound, expected) in [
            (ScalarValue::Signed(-2), -1.5, Less),
            (ScalarValue::Signed(-1), -1.5, Greater),
            (ScalarValue::Unsigned(9_007_199_254_740_995), 9_007_199_254_740_996.0, Less),
            (ScalarValue::Unsigned(9_007_199_254_740_993), 9_007_199_254_740_992.0, Greater),
            (ScalarValue::Signed(i64::MIN), i64::MIN as f64, Equal),
            (ScalarValue::Unsigned(u64::MAX), 18_446_744_073_709_551_616.0, Less),
            (ScalarValue::Signed(i64::MAX), 9_223_372_036_854_775_808.0, Less),
            (ScalarValue::Unsigned(0), -f64::MIN_POSITIVE, Greater),
            (ScalarValue::Unsigned(0), f64::MIN_POSITIVE, Less),
            (ScalarValue::Float(1.25), 1.5, Less),
        ] {
            assert_eq!(compare_bound(&value, bound).unwrap(), expected);
        }
    }
}
