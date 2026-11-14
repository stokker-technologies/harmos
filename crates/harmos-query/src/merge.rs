/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Merge Two Time-Ordered Sources Without A General Join

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, ArrayRef, RecordBatch, StructArray, UInt64Array, new_null_array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use arrow_select::{concat::concat, take::take};

// Local Imports
use harmos_domain::model::storage::StorageCursor;
use harmos_domain::model::storage::query::{TimeMerge, TimeMergeMode};
use harmos_domain::model::storage::query::scalar::ScalarValue;
use crate::batches::{BatchPage, BatchReader, QueryFuture};
use crate::transform::{FieldPath, FieldPathExecution};
use crate::{Error, Result};


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct TimeMergeReader {
    left: Input,
    right: Input,
    plan: TimeMerge,
    schema: SchemaRef,
    rows: NonZeroUsize,
    previous: Option<Row>,
    primary: Option<Row>,
    sample: Option<Row>,
    prepared: bool,
    active: Vec<Window>,
    emit: usize,
    done: bool,
}

struct Input {
    reader: Box<dyn BatchReader>,
    time: FieldPath,
    batch: Option<Arc<RecordBatch>>,
    offset: usize,
    complete: bool,
    previous: Option<i64>,
    current: Option<Row>,
}

#[derive(Clone)]
struct Row {
    batch: Arc<RecordBatch>,
    offset: usize,
    time: i64,
}

struct Window {
    row: Row,
    end: Option<i64>,
}

/* === Enums === */

enum Peek {
    Row(Row),
    Progress,
    End,
}

enum Step {
    Pair(Row, Option<Row>),
    Progress,
    End,
}

/* === Traits === */

/* === Implementations === */

impl TimeMergeReader {
    pub fn new(left: Box<dyn BatchReader>, right: Box<dyn BatchReader>, plan: TimeMerge, rows: NonZeroUsize) -> Result<Self> {
        if left.at() != right.at() {
            return Err(invalid("sources must retain the same cut"));
        }
        plan.primary_time.field(left.schema().as_ref())?;
        plan.secondary_time.field(right.schema().as_ref())?;
        if let TimeMergeMode::Range { end } = &plan.mode {
            end.field(left.schema().as_ref())?;
        }
        let schema = Arc::new(Schema::new(vec![
            Field::new("left", DataType::Struct(left.schema().fields().clone()), false),
            Field::new("right", DataType::Struct(right.schema().fields().clone()), matches!(plan.mode, TimeMergeMode::AtOrBefore)),
        ]));
        Ok(Self {
            left: Input::new(left, plan.primary_time.clone()),
            right: Input::new(right, plan.secondary_time.clone()),
            plan, schema, rows, previous: None, primary: None, sample: None,
            prepared: false, active: Vec::new(), emit: 0, done: false,
        })
    }

    async fn at_or_before(&mut self) -> Result<Step> {
        if self.primary.is_none() {
            match self.left.peek().await? {
                Peek::Row(row) => { self.left.consume(); self.primary = Some(row); }
                Peek::Progress => return Ok(Step::Progress),
                Peek::End => return Ok(Step::End),
            }
        }
        let primary = self.primary.as_ref().expect("retained primary row");
        for _ in 0..self.rows.get() {
            match self.right.peek().await? {
                Peek::Row(row) if row.time <= primary.time => {
                    self.right.consume();
                    self.previous = Some(row);
                }
                Peek::Progress => return Ok(Step::Progress),
                Peek::Row(_) | Peek::End => return Ok(Step::Pair(self.primary.take().expect("retained primary row"), self.previous.clone())),
            }
        }
        Ok(Step::Progress)
    }

    async fn intervals(&mut self) -> Result<Step> {
        let mut work = 0usize;
        loop {
            if self.sample.is_none() {
                match self.right.peek().await? {
                    Peek::Row(row) => { self.right.consume(); self.active.retain(|window| window.end.is_none_or(|end| end > row.time)); self.sample = Some(row); self.prepared = false; self.emit = 0; }
                    Peek::Progress => return Ok(Step::Progress),
                    Peek::End => return Ok(Step::End),
                }
            }
            let time = self.sample.as_ref().expect("retained stream row").time;
            while !self.prepared {
                match self.left.peek().await? {
                    Peek::Row(row) if row.time <= time => {
                        let end = match &self.plan.mode {
                            TimeMergeMode::Range { end } => {
                                let end = timestamp(end, &row.batch, row.offset)?;
                                if end < row.time { return Err(invalid("range end precedes its start")); }
                                Some(end)
                            }
                            TimeMergeMode::Intervals => { self.active.clear(); None }
                            TimeMergeMode::AtOrBefore => unreachable!("nearest mode uses its own sweep"),
                        };
                        self.left.consume();
                        if end.is_none_or(|end| end > time) { self.active.push(Window { row, end }); }
                        self.check_active()?;
                        work += 1;
                        if work >= self.rows.get() { return Ok(Step::Progress); }
                    }
                    Peek::Progress => return Ok(Step::Progress),
                    Peek::Row(_) | Peek::End => {
                        self.active.retain(|window| window.end.is_none_or(|end| end > time));
                        self.prepared = true;
                    }
                }
            }
            if let Some(window) = self.active.get(self.emit) {
                self.emit += 1;
                return Ok(Step::Pair(window.row.clone(), self.sample.clone()));
            }
            self.sample = None;
            work += 1;
            if work >= self.rows.get() { return Ok(Step::Progress); }
        }
    }

    fn check_active(&self) -> Result<()> {
        let mut batches = BTreeSet::new();
        let mut bytes = self.active.capacity().saturating_mul(std::mem::size_of::<Window>());
        for window in &self.active {
            if batches.insert(Arc::as_ptr(&window.row.batch) as usize) {
                bytes = bytes.saturating_add(window.row.batch.get_array_memory_size());
            }
        }
        if self.active.len() > self.plan.work.rows.get() || bytes > self.plan.work.bytes.get() {
            return Err(exhausted());
        }
        Ok(())
    }

    async fn advance(&mut self) -> Result<BatchPage> {
        let mut left = Vec::new();
        let mut right = Vec::new();
        while !self.done && left.len() < self.rows.get() {
            let step = match self.plan.mode {
                TimeMergeMode::AtOrBefore => self.at_or_before().await?,
                TimeMergeMode::Range { .. } | TimeMergeMode::Intervals => self.intervals().await?,
            };
            match step {
                Step::Pair(primary, secondary) => { left.push(Some(primary)); right.push(secondary); }
                Step::Progress => break,
                Step::End => self.done = true,
            }
        }
        let batch = if left.is_empty() {
            RecordBatch::new_empty(Arc::clone(&self.schema))
        } else {
            let left_type = self.schema.field(0).data_type();
            let right_type = self.schema.field(1).data_type();
            RecordBatch::try_new(Arc::clone(&self.schema), vec![gather(&left, left_type)?, gather(&right, right_type)?])
                .map_err(|_| invalid("cannot assemble matched rows"))?
        };
        if batch.get_array_memory_size() > self.plan.work.bytes.get() { return Err(exhausted()); }
        Ok(BatchPage { batch, complete: self.done })
    }
}

impl BatchReader for TimeMergeReader {
    fn schema(&self) -> SchemaRef { Arc::clone(&self.schema) }
    fn at(&self) -> &StorageCursor { self.left.reader.at() }
    fn next(&mut self) -> QueryFuture<'_, BatchPage> { Box::pin(self.advance()) }
}

impl Input {
    fn new(reader: Box<dyn BatchReader>, time: FieldPath) -> Self {
        Self { reader, time, batch: None, offset: 0, complete: false, previous: None, current: None }
    }

    async fn peek(&mut self) -> Result<Peek> {
        if let Some(row) = &self.current { return Ok(Peek::Row(row.clone())); }
        if self.batch.as_ref().is_none_or(|batch| self.offset >= batch.num_rows()) {
            if self.complete { return Ok(Peek::End); }
            let page = self.reader.next().await?;
            self.complete = page.complete;
            self.offset = 0;
            self.batch = Some(Arc::new(page.batch));
            if self.batch.as_ref().expect("loaded batch").num_rows() == 0 {
                return Ok(if self.complete { Peek::End } else { Peek::Progress });
            }
        }
        let batch = self.batch.as_ref().expect("loaded batch");
        let time = timestamp(&self.time, batch, self.offset)?;
        if self.previous.is_some_and(|previous| time < previous) {
            return Err(invalid("inputs must be ordered by nondecreasing time"));
        }
        self.previous = Some(time);
        let row = Row { batch: Arc::clone(batch), offset: self.offset, time };
        self.offset += 1;
        self.current = Some(row.clone());
        Ok(Peek::Row(row))
    }

    fn consume(&mut self) { self.current = None; }
}

/* === Public Functions === */

/* === Private Functions === */

fn timestamp(path: &FieldPath, batch: &RecordBatch, row: usize) -> Result<i64> {
    match path.value(batch, row)? {
        ScalarValue::Datetime(value) => i64::try_from(value).map_err(|_| invalid("timestamp overflows nanoseconds")),
        ScalarValue::Signed(value) => Ok(value),
        ScalarValue::Unsigned(value) => i64::try_from(value).map_err(|_| invalid("timestamp overflows nanoseconds")),
        _ => Err(invalid("time columns require non-null timestamps or integer nanoseconds")),
    }
}

fn gather(rows: &[Option<Row>], data_type: &DataType) -> Result<ArrayRef> {
    let mut parts = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        let mut end = start + 1;
        match &rows[start] {
            Some(first) => {
                while end < rows.len() && rows[end].as_ref().is_some_and(|row| Arc::ptr_eq(&row.batch, &first.batch)) { end += 1; }
                let indices = UInt64Array::from_iter_values(rows[start..end].iter().map(|row| row.as_ref().expect("non-null run").offset as u64));
                let structure = StructArray::from(first.batch.as_ref().clone());
                parts.push(take(&structure, &indices, None).map_err(|_| invalid("cannot select matched rows"))?);
            }
            None => {
                while end < rows.len() && rows[end].is_none() { end += 1; }
                parts.push(new_null_array(data_type, end - start));
            }
        }
        start = end;
    }
    let arrays = parts.iter().map(|array| array.as_ref()).collect::<Vec<&dyn Array>>();
    concat(&arrays).map_err(|_| invalid("cannot concatenate matched rows"))
}

fn invalid(reason: &'static str) -> Error { Error::InvalidInput { field: "time merge", reason } }

fn exhausted() -> Error { Error::ResourceExhausted { resource: "time merge work" } }

/* === Tests === */
