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
use std::cmp;
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{iter, mem};

// Third Party Imports
use arrow_array::{Array, LargeStringArray, RecordBatch, RecordBatchOptions, StringArray, StringViewArray, UInt64Array};
use arrow_schema::Field;
use arrow_schema::{DataType, TimeUnit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::scalar::ScalarValue;
use super::transform::{
    FieldPath, ProjectExecution,
    project::{Project, Projection},
};
use crate::ObjectEntry;
use crate::StoredRecord;
use crate::StoredRow;
use crate::arrow::ArrowBatch;
use crate::{BackendFailure, Error, MemorySize, Result, SchemaKey};
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

    /// Nonempty Case-Sensitive TEXT Prefix; Numeric Bounds Must Be Absent.
    pub prefix: Option<String>,
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

    /// Payload Projection Visits, Including Separate Predicate and Output Reads.
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

impl ColumnPredicate {
    /// Refuse a Selection Whose Kind Does Not Match the Predicate Column.
    pub fn validate_type(&self, data_type: &DataType) -> Result<()> {
        let text = matches!(data_type, DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View);
        if self.prefix.is_some() && !text {
            return Err(invalid("select a TEXT column for a text prefix, or use numeric bounds"));
        }
        if self.prefix.is_none()
            && !matches!(
                data_type,
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
            )
        {
            return Err(invalid("select a numeric column for numeric bounds, or use a text prefix"));
        }
        Ok(())
    }
}

impl MemorySize for ColumnSource {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.schema.memory_size())
    }
}

impl MemorySize for ColumnPredicate {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.column.memory_size()).saturating_add(self.prefix.memory_size())
    }
}

impl MemorySize for TimeBounds {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.column.memory_size())
    }
}

impl MemorySize for ArrowPage {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.batch.get_array_memory_size()).saturating_add(self.columns_read.memory_size())
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
        let encoded = serde_json::to_vec(&(&self.source, &self.columns, &self.predicate, &self.time, self.start))
            .map_err(|source| Error::backend("encode column plan", BackendFailure::Permanent, source))?;
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
            if let Some(prefix) = &predicate.prefix {
                if predicate.lower.is_some() || predicate.upper.is_some() {
                    return Err(invalid("choose either a text prefix or numeric bounds; remove the other selection"));
                }
                if prefix.is_empty() {
                    return Err(invalid("supply a nonempty text prefix or omit the predicate"));
                }
            }
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
        ColumnKind::Records => crate::source::records(schema, iter::empty())?,
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
    let output_indices = read
        .columns
        .iter()
        .map(|name| batch.schema().index_of(name).map_err(|_| invalid("requested column is unavailable")))
        .collect::<Result<Vec<_>>>()?;
    let output = batch.project(&output_indices).map_err(|_| invalid("column projection failed"))?;
    finish_scan(snapshot, read, batch, evidence, cancel, |indices| Ok((take(&output, indices)?, 0)))
}

/// Evaluate Predicate Columns Before Loading Output Columns for Matching Rows.
pub fn finish_scan(
    snapshot: &BackendSnapshot, read: &ArrowScan, batch: &RecordBatch, evidence: ScanEvidence, cancel: &Cancellation,
    load: impl FnOnce(&[u64]) -> Result<(RecordBatch, usize)>,
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
    if let Some(predicate) = &read.predicate {
        let array = batch.column_by_name(&predicate.column).ok_or_else(|| invalid("select an available predicate column"))?;
        predicate.validate_type(array.data_type())?;
    }
    if let Some(time) = &read.time {
        let array = batch.column_by_name(&time.column).ok_or_else(|| invalid("select an available time column"))?;
        if !matches!(array.data_type(), DataType::Int64 | DataType::Timestamp(TimeUnit::Nanosecond, _)) {
            return Err(invalid("time column must be Int64 or nanosecond timestamp"));
        }
    }
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
    let (mut batch, loaded_payload_rows) = load(&indices)?;
    if batch.num_rows() != indices.len() || batch.schema().fields().iter().map(|field| field.name()).ne(read.columns.iter()) {
        return Err(invalid("load the selected output columns in scan order"));
    }
    if page_bytes(&batch)? > read.limits.bytes.get() {
        let output_indices = (0..indices.len() as u64).collect::<Vec<_>>();
        let mut low = 0;
        let mut high = indices.len();
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            let candidate = take(&batch, &output_indices[..middle])?;
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
        batch = take(&batch, &output_indices[..low])?;
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
        payload_rows: payload_rows.saturating_add(loaded_payload_rows),
    })
}

pub fn page_bytes(batch: &RecordBatch) -> Result<usize> {
    let ipc = ArrowBatch::try_new(batch.clone())?;
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
        if let Some(prefix) = &predicate.prefix {
            if array.is_null(row) {
                return Ok(false);
            }
            let value = match array.data_type() {
                DataType::Utf8 => array.as_any().downcast_ref::<StringArray>().expect("validated text column").value(row),
                DataType::LargeUtf8 => array.as_any().downcast_ref::<LargeStringArray>().expect("validated text column").value(row),
                DataType::Utf8View => array.as_any().downcast_ref::<StringViewArray>().expect("validated text column").value(row),
                _ => return Err(invalid("select a TEXT column for a text prefix, or use numeric bounds")),
            };
            return Ok(value.starts_with(prefix));
        }
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

fn compare_bound(value: &ScalarValue, bound: f64) -> Result<cmp::Ordering> {
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
        return Ok(cmp::Ordering::Less);
    }
    if bound < i64::MIN as f64 {
        return Ok(cmp::Ordering::Greater);
    }
    let order = integer.cmp(&(bound as i128));
    if order != cmp::Ordering::Equal {
        return Ok(order);
    }
    Ok(if bound.fract() > 0.0 {
        cmp::Ordering::Less
    } else if bound.fract() < 0.0 {
        cmp::Ordering::Greater
    } else {
        cmp::Ordering::Equal
    })
}

pub fn take(batch: &RecordBatch, indices: &[u64]) -> Result<RecordBatch> {
    let indices = UInt64Array::from(indices.to_vec());
    let columns = batch
        .columns()
        .iter()
        .map(|column| arrow_select::take::take(column.as_ref(), &indices, None).map_err(|_| invalid("Arrow row selection failed")))
        .collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new_with_options(batch.schema(), columns, &RecordBatchOptions::new().with_row_count(Some(indices.len())))
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

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, PayloadEncoding, StoreHistory};
    use arrow_array::{Float64Array, StringArray};
    use arrow_schema::Schema;
    use cmp::Ordering::{Equal, Greater, Less};
    use harmos_domain::model::storage::StorageCursor;
    use std::num::{NonZeroU32, NonZeroUsize};

    fn text_scan() -> (BackendSnapshot, ArrowScan, RecordBatch) {
        let snapshot = BackendSnapshot {
            at: StorageCursor {
                history: StoreHistory {
                    store: Identifier::new("test").unwrap(),
                    epoch: Identifier::new("epoch").unwrap(),
                },
                sequence: 5,
            },
            token: vec![1],
        };
        let read = ArrowScan {
            source: ColumnSource {
                kind: ColumnKind::Objects,
                schema: SchemaKey {
                    id: Identifier::new("example").unwrap(),
                    version: NonZeroU32::MIN,
                    payload_encoding: PayloadEncoding::PostcardV1,
                },
            },
            columns: vec!["key".into()],
            predicate: Some(ColumnPredicate {
                column: "key".into(),
                lower: None,
                upper: None,
                prefix: Some("é/".into()),
            }),
            time: None,
            start: 0,
            after: None,
            limits: ReadLimits {
                rows: NonZeroUsize::new(2).unwrap(),
                bytes: NonZeroUsize::new(8192).unwrap(),
            },
            work: ReadLimits {
                rows: NonZeroUsize::new(5).unwrap(),
                bytes: NonZeroUsize::new(8192).unwrap(),
            },
        };
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("_storage.position", DataType::UInt64, false),
                Field::new("key", DataType::Utf8, true),
                Field::new("number", DataType::Float64, false),
            ])),
            vec![
                Arc::new(UInt64Array::from(vec![1, 2, 3, 4, 5])),
                Arc::new(StringArray::from(vec![Some("é/first"), Some("e/other"), None, Some("é/last"), Some("éx")])),
                Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
            ],
        )
        .unwrap();
        (snapshot, read, batch)
    }

    fn evidence() -> ScanEvidence {
        ScanEvidence {
            complete: true,
            bytes: 0,
            columns: vec![],
            payload_rows: 0,
        }
    }

    #[test]
    fn text_prefix_matches_unicode_and_excludes_nulls() {
        let (snapshot, read, batch) = text_scan();
        let page = finish_page(&snapshot, &read, &batch, evidence(), &Cancellation::default()).unwrap();
        let keys = page.batch.column(0).as_any().downcast_ref::<StringArray>().unwrap();
        assert_eq!(keys.iter().collect::<Vec<_>>(), vec![Some("é/first"), Some("é/last")]);
        assert_eq!(page.scanned, 4);
        assert_eq!(page.next.unwrap().position, 4);
    }

    #[test]
    fn text_prefix_refuses_empty_and_mixed_numeric_selections() {
        let (_, mut read, _) = text_scan();
        read.predicate.as_mut().unwrap().prefix = Some(String::new());
        assert!(matches!(read.validate(), Err(Error::InvalidInput { .. })));
        read.predicate.as_mut().unwrap().prefix = Some("a".into());
        for (lower, upper) in [(Some(1.0), None), (None, Some(2.0)), (Some(1.0), Some(2.0))] {
            read.predicate.as_mut().unwrap().lower = lower;
            read.predicate.as_mut().unwrap().upper = upper;
            assert!(matches!(read.validate(), Err(Error::InvalidInput { .. })));
        }
    }

    #[test]
    fn text_prefix_refuses_non_text_columns_even_when_empty() {
        let (snapshot, mut read, batch) = text_scan();
        read.predicate.as_mut().unwrap().column = "number".into();
        for batch in [batch.clone(), batch.slice(0, 0)] {
            assert!(matches!(finish_page(&snapshot, &read, &batch, evidence(), &Cancellation::default()), Err(Error::InvalidInput { .. })));
        }
    }

    #[test]
    fn numeric_predicate_refuses_text_columns_even_without_bounds() {
        let (snapshot, mut read, batch) = text_scan();
        read.predicate.as_mut().unwrap().prefix = None;
        assert!(matches!(finish_page(&snapshot, &read, &batch, evidence(), &Cancellation::default()), Err(Error::InvalidInput { .. })));
    }

    #[test]
    fn text_prefix_continuation_refuses_changed_predicate_and_snapshot() {
        let (snapshot, mut read, batch) = text_scan();
        read.after = finish_page(&snapshot, &read, &batch, evidence(), &Cancellation::default()).unwrap().next;
        assert!(read.position(&snapshot).is_ok());
        let mut changed = read.clone();
        changed.predicate.as_mut().unwrap().prefix = Some("e/".into());
        assert!(changed.position(&snapshot).is_err());
        let mut changed = snapshot;
        changed.token.push(2);
        assert!(read.position(&changed).is_err());
    }

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
