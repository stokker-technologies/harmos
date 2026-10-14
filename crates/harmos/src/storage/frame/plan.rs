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
use std::cmp::Ordering;
use std::num::NonZeroUsize;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::storage::codec::{
    Both, Code, Count, Flag, Index, Many, Nested, Optional, Repeated, Wrap, coded, product,
    variants,
};
use crate::storage::read_selection::{ReadKind, ReadSelection};
use crate::storage::reduce::scalar::{Agg, ScalarKind, ScalarValue};
use crate::{Aggregate, Error, Identifier, MemorySize, Result, schema};

/* === Constants === */
pub(crate) const MAX_SCANS: usize = 16;
pub(crate) const MAX_JOINS: usize = 64;
pub(crate) const MAX_GROUP_BYS: usize = 64;
pub(crate) const MAX_WIDTH: usize = 64;
pub(crate) const MAX_GROUPS: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scan {
    pub selection: ReadSelection,
    pub columns: Vec<ScalarField>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsOf {
    pub left: usize,
    pub right: usize,
    pub by: Vec<(usize, usize)>,
    pub on: (usize, usize),
    pub strategy: AsOfStrategy,
    pub right_filter: Vec<Predicate>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LastBy {
    pub member: usize,
    pub order: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupBy {
    pub source: usize,
    pub filter: Vec<Predicate>,
    pub keys: Vec<usize>,
    pub groups: Option<Vec<Vec<ScalarValue>>>,
    pub last_by: Option<LastBy>,
    pub aggs: Vec<(Expr, Agg)>,
    pub outer: Option<Vec<Agg>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub scans: Vec<Scan>,
    pub as_ofs: Vec<AsOf>,
    pub group_bys: Vec<GroupBy>,
    pub max_rows: NonZeroUsize,
    pub max_groups: NonZeroUsize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRow {
    pub key: Vec<ScalarValue>,
    pub values: Vec<ScalarValue>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupOutput {
    pub groups: Vec<GroupRow>,
    pub outer: Option<Vec<ScalarValue>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collected {
    pub outputs: Vec<GroupOutput>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explain {
    pub peak_bytes: usize,
    pub page_bytes: usize,
    pub group_state_bytes: usize,
    pub median_scratch_bytes: usize,
    pub output_bytes: usize,
    pub text_bytes: usize,
}

/* === Enums === */
// Filters are combined with AND. Membership alternatives are bounded to 64.
// Presence excludes both missing fields and explicit null values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Predicate {
    Compare {
        expr: Expr,
        compare: Compare,
        value: ScalarValue,
    },
    In {
        expr: Expr,
        values: Vec<ScalarValue>,
    },
    Exists {
        expr: Expr,
        present: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarPath {
    Payload(Vec<Identifier>),
    SourceTime,
    CaptureOrder,
    Scope,
    Stream,
    Item,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    Col(usize),
    AsOf { join: usize, col: usize },
    Elapsed(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsOfStrategy {
    Backward { present: usize },
    Previous,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarField {
    pub path: ScalarPath,
    pub kind: ScalarKind,
}

/* === Traits === */

/* === Implementations === */
impl Frame {
    pub fn new(max_rows: NonZeroUsize, max_groups: NonZeroUsize) -> Self {
        Self {
            scans: Vec::new(),
            as_ofs: Vec::new(),
            group_bys: Vec::new(),
            max_rows,
            max_groups,
        }
    }

    pub fn scan(&mut self, selection: ReadSelection, columns: Vec<ScalarField>) -> usize {
        self.scans.push(Scan { selection, columns });
        self.scans.len() - 1
    }

    pub fn join_asof(&mut self, as_of: AsOf) -> usize {
        self.as_ofs.push(as_of);
        self.as_ofs.len() - 1
    }

    pub fn shift(&mut self, source: usize, by: Vec<usize>, on: usize) -> usize {
        self.join_asof(AsOf {
            left: source,
            right: source,
            by: by.into_iter().map(|column| (column, column)).collect(),
            on: (on, on),
            strategy: AsOfStrategy::Previous,
            right_filter: Vec::new(),
        })
    }

    pub fn group_by(&mut self, group_by: GroupBy) -> usize {
        self.group_bys.push(group_by);
        self.group_bys.len() - 1
    }

    pub fn check(&self) -> Result<()> {
        if self.max_groups.get() > MAX_GROUPS {
            return Err(invalid("group bound exceeds 4096"));
        }
        if self.scans.is_empty()
            || self.scans.len() > MAX_SCANS
            || self.as_ofs.len() > MAX_JOINS
            || self.group_bys.is_empty()
            || self.group_bys.len() > MAX_GROUP_BYS
        {
            return Err(invalid("frame exceeds scan, join or group bound"));
        }
        for scan in &self.scans {
            if scan.selection.record_filter.is_some() {
                return Err(invalid("frame predicates belong to group or join filters"));
            }
            if scan.columns.is_empty() || scan.columns.len() > MAX_WIDTH {
                return Err(invalid("scan needs 1..64 typed columns"));
            }
            if !matches!(scan.selection.kind, ReadKind::Records | ReadKind::Stream) {
                return Err(invalid("frames scan records or stream rows"));
            }
            for column in &scan.columns {
                column.check()?;
                if column.path == ScalarPath::Stream && scan.selection.kind != ReadKind::Stream {
                    return Err(invalid("stream column requires a stream scan"));
                }
            }
            if scan
                .selection
                .through
                .is_some_and(|end| end < scan.selection.after)
            {
                return Err(invalid("invalid scan cut"));
            }
        }
        for join in &self.as_ofs {
            self.check_join(join)?;
        }
        for group_by in &self.group_bys {
            self.check_group_by(group_by)?;
        }
        Ok(())
    }

    fn check_join(&self, join: &AsOf) -> Result<()> {
        self.column(join.left, join.on.0)?;
        self.column(join.right, join.on.1)?;
        if self.kind(join.left, join.on.0) != ScalarKind::Datetime
            || self.kind(join.right, join.on.1) != ScalarKind::Datetime
        {
            return Err(invalid("as-of join requires normalized datetime columns"));
        }
        if join.by.is_empty() || join.by.len() > MAX_WIDTH || join.right_filter.len() > MAX_WIDTH {
            return Err(invalid("as-of join requires explicit bounded keys"));
        }
        for &(left, right) in &join.by {
            self.column(join.left, left)?;
            self.column(join.right, right)?;
            if self.kind(join.left, left) != self.kind(join.right, right) {
                return Err(invalid("as-of key types differ"));
            }
        }
        if let AsOfStrategy::Backward { present } = join.strategy {
            self.column(join.right, present)?;
        }
        for predicate in &join.right_filter {
            if !matches!(predicate.expr(), Expr::Col(_)) {
                return Err(invalid("right filters must use original source columns"));
            }
            self.predicate(join.right, predicate)?;
        }
        Ok(())
    }

    fn check_group_by(&self, group_by: &GroupBy) -> Result<()> {
        if group_by.source >= self.scans.len()
            || group_by.keys.len() > MAX_WIDTH
            || group_by.filter.len() > MAX_WIDTH
            || group_by.aggs.is_empty()
            || group_by.aggs.len() > MAX_WIDTH
        {
            return Err(invalid("invalid group-by dimensions"));
        }
        for &column in &group_by.keys {
            self.column(group_by.source, column)?;
        }
        if let Some(last_by) = &group_by.last_by {
            self.column(group_by.source, last_by.member)?;
            self.column(group_by.source, last_by.order)?;
        }
        for predicate in &group_by.filter {
            self.predicate(group_by.source, predicate)?;
        }
        for (expr, agg) in &group_by.aggs {
            self.expr(group_by.source, expr)?;
            check_agg(*agg, self.expr_kind(group_by.source, expr))?;
        }
        group_by.join()?;
        if let Some(outer) = &group_by.outer {
            if outer.len() != group_by.aggs.len() {
                return Err(invalid("outer aggregates must match inner aggregates"));
            }
            for ((expr, inner), &agg) in group_by.aggs.iter().zip(outer) {
                let kind = match inner {
                    Agg::Len | Agg::Of(Aggregate::Count) => ScalarKind::Unsigned,
                    Agg::Of(Aggregate::Sum | Aggregate::Mean | Aggregate::Median) => {
                        ScalarKind::Exact
                    }
                    Agg::Of(Aggregate::Rms | Aggregate::Std | Aggregate::Percentile(_)) => {
                        ScalarKind::Float
                    }
                    Agg::Last | Agg::Of(Aggregate::Min | Aggregate::Max) => {
                        self.expr_kind(group_by.source, expr)
                    }
                };
                check_agg(agg, kind)?;
            }
        }
        if let Some(groups) = &group_by.groups {
            if groups.len() > self.max_groups.get() {
                return Err(invalid("explicit groups exceed limit"));
            }
            for (index, key) in groups.iter().enumerate() {
                if key.len() != group_by.keys.len() {
                    return Err(invalid("group key width differs"));
                }
                for (value, &column) in key.iter().zip(&group_by.keys) {
                    self.value(group_by.source, column, value)?;
                }
                if groups[..index].contains(key) {
                    return Err(invalid("duplicate explicit group"));
                }
            }
        }
        Ok(())
    }

    fn column(&self, scan: usize, column: usize) -> Result<()> {
        if self
            .scans
            .get(scan)
            .and_then(|scan| scan.columns.get(column))
            .is_none()
        {
            return Err(invalid("scan column out of range"));
        }
        Ok(())
    }

    fn kind(&self, scan: usize, column: usize) -> ScalarKind {
        self.scans[scan].columns[column].kind
    }

    fn value(&self, scan: usize, column: usize, value: &ScalarValue) -> Result<()> {
        value.check()?;
        if value
            .kind()
            .is_some_and(|kind| kind != self.kind(scan, column))
        {
            return Err(invalid("scalar differs from scan column type"));
        }
        Ok(())
    }

    fn expr(&self, scan: usize, expr: &Expr) -> Result<()> {
        match *expr {
            Expr::Col(column) => self.column(scan, column),
            Expr::AsOf { join, col } => {
                let as_of = self
                    .as_ofs
                    .get(join)
                    .ok_or(invalid("as-of join out of range"))?;
                if as_of.left != scan {
                    return Err(invalid("as-of join has a different left scan"));
                }
                self.column(as_of.right, col)
            }
            Expr::Elapsed(join) => {
                let as_of = self
                    .as_ofs
                    .get(join)
                    .ok_or(invalid("as-of join out of range"))?;
                if as_of.left != scan || as_of.strategy != AsOfStrategy::Previous {
                    return Err(invalid("elapsed requires a previous join on the same scan"));
                }
                Ok(())
            }
        }
    }

    fn expr_kind(&self, scan: usize, expr: &Expr) -> ScalarKind {
        match *expr {
            Expr::Col(column) => self.kind(scan, column),
            Expr::AsOf { join, col } => self.kind(self.as_ofs[join].right, col),
            Expr::Elapsed(_) => ScalarKind::Exact,
        }
    }

    fn predicate(&self, scan: usize, predicate: &Predicate) -> Result<()> {
        self.expr(scan, predicate.expr())?;
        let selected = self.expr_kind(scan, predicate.expr());
        predicate.check_kind(selected)
    }
}
impl GroupBy {
    pub fn new(source: usize) -> Self {
        Self {
            source,
            filter: Vec::new(),
            keys: Vec::new(),
            groups: None,
            last_by: None,
            aggs: Vec::new(),
            outer: None,
        }
    }

    pub fn keys(mut self, keys: Vec<usize>) -> Self {
        self.keys = keys;
        self
    }

    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.filter.push(predicate);
        self
    }

    pub fn agg(mut self, expr: Expr, agg: impl Into<Agg>) -> Self {
        self.aggs.push((expr, agg.into()));
        self
    }

    pub fn join(&self) -> Result<Option<usize>> {
        let mut joined = None;
        let exprs = self
            .filter
            .iter()
            .map(Predicate::expr)
            .chain(self.aggs.iter().map(|(expr, _)| expr));
        for expr in exprs {
            let join = match *expr {
                Expr::Col(_) => continue,
                Expr::AsOf { join, .. } | Expr::Elapsed(join) => join,
            };
            if joined.is_some_and(|joined| joined != join) {
                return Err(invalid("a group-by may read at most one as-of join"));
            }
            joined = Some(join);
        }
        Ok(joined)
    }
}
impl Expr {
    pub fn diff(join: usize) -> Self {
        Self::Elapsed(join)
    }
}
impl Collected {
    pub fn check(&self, frame: &Frame) -> Result<()> {
        frame.check()?;
        if self.outputs.len() != frame.group_bys.len() {
            return Err(invalid("output count differs from frame"));
        }
        for (output, group_by) in self.outputs.iter().zip(&frame.group_bys) {
            if output.groups.len() > frame.max_groups.get()
                || output.outer.as_ref().map(Vec::len) != group_by.outer.as_ref().map(Vec::len)
            {
                return Err(invalid("groups or outer values differ"));
            }
            if let Some(keys) = &group_by.groups {
                if output.groups.len() != keys.len()
                    || output
                        .groups
                        .iter()
                        .zip(keys)
                        .any(|(group, key)| group.key != *key)
                {
                    return Err(invalid("explicit empty groups or ordering were lost"));
                }
            } else if group_by.keys.is_empty() && output.groups.len() != 1 {
                return Err(invalid(
                    "ungrouped aggregate requires one result including empty input",
                ));
            }
            let mut previous: Option<&Vec<ScalarValue>> = None;
            for group in &output.groups {
                if group.key.len() != group_by.keys.len()
                    || group.values.len() != group_by.aggs.len()
                {
                    return Err(invalid("group shape differs"));
                }
                for (value, &column) in group.key.iter().zip(&group_by.keys) {
                    frame.value(group_by.source, column, value)?;
                }
                if group_by.groups.is_none() && !group_by.keys.is_empty() {
                    if let Some(previous) = previous
                        && !compare_keys(previous, &group.key)?.is_lt()
                    {
                        return Err(invalid("group ordering differs"));
                    }
                    previous = Some(&group.key);
                }
                for (value, (_, agg)) in group.values.iter().zip(&group_by.aggs) {
                    check_output(value, *agg)?;
                }
            }
            if let (Some(values), Some(aggs)) = (&output.outer, &group_by.outer) {
                for (value, agg) in values.iter().zip(aggs) {
                    check_output(value, *agg)?;
                }
            }
        }
        Ok(())
    }
}
impl MemorySize for Collected {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(heap(&self.outputs))
    }
}
impl MemorySize for GroupOutput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.groups))
            .saturating_add(heap(&self.outer))
    }
}
impl MemorySize for GroupRow {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.key))
            .saturating_add(heap(&self.values))
    }
}
impl Predicate {
    pub(crate) fn check_kind(&self, selected: ScalarKind) -> Result<()> {
        let numeric = |kind| {
            matches!(
                kind,
                ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Exact
            )
        };
        let check = |value: &ScalarValue| {
            value.check()?;
            if value
                .kind()
                .is_some_and(|kind| kind != selected && !(numeric(kind) && numeric(selected)))
            {
                return Err(invalid("predicate differs from selected expression type"));
            }
            Ok(())
        };
        match self {
            Predicate::Compare { value, .. } => check(value),
            Predicate::In { values, .. } => {
                if values.len() > MAX_WIDTH {
                    return Err(invalid("membership exceeds 64 alternatives"));
                }
                for value in values {
                    check(value)?;
                }
                Ok(())
            }
            Predicate::Exists { .. } => Ok(()),
        }
    }
    pub fn expr(&self) -> &Expr {
        match self {
            Self::Compare { expr, .. } | Self::In { expr, .. } | Self::Exists { expr, .. } => expr,
        }
    }
}
impl MemorySize for Predicate {
    fn memory_size(&self) -> usize {
        let allocated = match self {
            Self::Compare { value, .. } => heap(value),
            Self::In { values, .. } => heap(values),
            Self::Exists { .. } => 0,
        };
        std::mem::size_of::<Self>().saturating_add(allocated)
    }
}
impl MemorySize for Frame {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.scans))
            .saturating_add(heap(&self.as_ofs))
            .saturating_add(heap(&self.group_bys))
    }
}
impl MemorySize for Scan {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.columns))
            .saturating_add(heap(&self.selection))
    }
}
impl MemorySize for AsOf {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.by
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(usize, usize)>()),
            )
            .saturating_add(heap(&self.right_filter))
    }
}
impl MemorySize for GroupBy {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.filter))
            .saturating_add(heap(&self.groups))
            .saturating_add(
                self.keys
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(
                self.aggs
                    .iter()
                    .map(|_| std::mem::size_of::<(Expr, Agg)>())
                    .fold(0usize, usize::saturating_add),
            )
            .saturating_add(self.outer.as_ref().map_or(0, |aggs| {
                aggs.capacity().saturating_mul(std::mem::size_of::<Agg>())
            }))
    }
}
impl ScalarField {
    pub fn check(&self) -> Result<()> {
        if let ScalarPath::Payload(path) = &self.path {
            if path.is_empty()
                || path.len() > schema::MAX_DEPTH
                || path
                    .iter()
                    .any(|name| name.as_str().len() > schema::MAX_NAME_BYTES)
            {
                return Err(invalid("field path exceeds traversal bounds"));
            }
        } else {
            let expected = match self.path {
                ScalarPath::SourceTime => ScalarKind::Datetime,
                ScalarPath::CaptureOrder => ScalarKind::Unsigned,
                _ => ScalarKind::Text,
            };
            if self.kind != expected {
                return Err(invalid("metadata field has different scalar kind"));
            }
        }
        Ok(())
    }

    pub fn check_schema(&self, view: schema::graph::View<'_>) -> Result<()> {
        self.check()?;
        let ScalarPath::Payload(path) = &self.path else {
            return Ok(());
        };
        let mut view = view;
        for (index, name) in path.iter().enumerate() {
            let field = view
                .fields()
                .iter()
                .find(|field| field.name == name.as_str())
                .ok_or(invalid("unknown schema field"))?;
            if matches!(
                field.cardinality,
                schema::Cardinality::Repeated | schema::Cardinality::Variant
            ) {
                return Err(invalid("field path crosses repeated or variant value"));
            }
            if index + 1 < path.len() {
                view = view
                    .message(field)
                    .map_err(|_| invalid("invalid schema reference"))?
                    .ok_or(invalid("field path crosses scalar"))?;
                continue;
            }
            let kind = match field.kind {
                schema::graph::Kind::Bool => ScalarKind::Bool,
                schema::graph::Kind::I32 | schema::graph::Kind::I64 => ScalarKind::Signed,
                schema::graph::Kind::U32 | schema::graph::Kind::U64 => ScalarKind::Unsigned,
                schema::graph::Kind::F32 | schema::graph::Kind::F64 => ScalarKind::Float,
                schema::graph::Kind::String => ScalarKind::Text,
                _ => return Err(invalid("selected field is not a supported scalar")),
            };
            if kind != self.kind {
                return Err(invalid("field type differs from schema"));
            }
        }
        Ok(())
    }
}
impl MemorySize for ScalarField {
    fn memory_size(&self) -> usize {
        // The Path Enum Is Inline; Only Its Owned Names Are Counted Beyond It.
        std::mem::size_of::<Self>().saturating_add(heap(&self.path))
    }
}

impl MemorySize for ScalarPath {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::Payload(path) => heap(path),
            _ => 0,
        })
    }
}
impl MemorySize for Expr {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl MemorySize for AsOfStrategy {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl MemorySize for LastBy {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl MemorySize for Explain {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

// Wire Codecs: Indices Travel As uint64 And Decode Without Truncation; No Type Recurses.
coded!(Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge
});
variants!(Expr {
    0, 1, "col": Self::Col(column) => { column: Index => 1 } => Self::Col(column);
    1, 2, "as_of": Self::AsOf { join, col } => { join: Index => 1, col: Index => 2 } => Self::AsOf { join, col };
    2, 3, "elapsed": Self::Elapsed(join) => { join: Index => 1 } => Self::Elapsed(join);
});
variants!(AsOfStrategy {
    0, 1, "backward": Self::Backward { present } => { present: Index => 1 } => Self::Backward { present };
    1, 2, "previous": Self::Previous => {} => Self::Previous;
});
variants!(ScalarPath {
    0, 1, "payload": Self::Payload(names) => { names: Repeated<Identifier> => 1 } => Self::Payload(names);
    1, 2, "source_time": Self::SourceTime => {} => Self::SourceTime;
    2, 3, "capture_order": Self::CaptureOrder => {} => Self::CaptureOrder;
    3, 4, "scope": Self::Scope => {} => Self::Scope;
    4, 5, "stream": Self::Stream => {} => Self::Stream;
    5, 6, "item": Self::Item => {} => Self::Item;
});
product!(ScalarField { path: Nested<ScalarPath> => 1, kind: Code<ScalarKind> => 2 });
product!(Scan { selection: Nested<ReadSelection> => 1, columns: Repeated<ScalarField> => 2 });
variants!(Predicate {
    0, 1, "compare": Self::Compare { expr, compare, value } => { expr: Nested<Expr> => 1, compare: Code<Compare> => 2, value: Nested<ScalarValue> => 3 } => Self::Compare { expr, compare, value };
    1, 2, "in": Self::In { expr, values } => { expr: Nested<Expr> => 1, values: Repeated<ScalarValue> => 2 } => Self::In { expr, values };
    2, 3, "exists": Self::Exists { expr, present } => { expr: Nested<Expr> => 1, present: Flag => 2 } => Self::Exists { expr, present };
});
product!(AsOf {
    left: Index => 1,
    right: Index => 2,
    by: Many<Both<Index, Index>> => 3,
    on: Both<Index, Index> => 4,
    strategy: Nested<AsOfStrategy> => 5,
    right_filter: Repeated<Predicate> => 6,
});
product!(LastBy { member: Index => 1, order: Index => 2 });
product!(GroupBy {
    source: Index => 1,
    filter: Repeated<Predicate> => 2,
    keys: Many<Index> => 3,
    groups: Optional<Wrap<Many<Repeated<ScalarValue>>>> => 4,
    last_by: Optional<Nested<LastBy>> => 5,
    aggs: Many<Both<Nested<Expr>, Nested<Agg>>> => 6,
    outer: Optional<Wrap<Many<Nested<Agg>>>> => 7,
});
product!(Frame {
    scans: Repeated<Scan> => 1,
    as_ofs: Repeated<AsOf> => 2,
    group_bys: Repeated<GroupBy> => 3,
    max_rows: Count => 4,
    max_groups: Count => 5,
});
product!(GroupRow { key: Repeated<ScalarValue> => 1, values: Repeated<ScalarValue> => 2 });
product!(GroupOutput {
    groups: Repeated<GroupRow> => 1,
    outer: Optional<Wrap<Repeated<ScalarValue>>> => 2,
});
product!(Collected { outputs: Repeated<GroupOutput> => 1 });
product!(Explain {
    peak_bytes: Index => 1,
    page_bytes: Index => 2,
    group_state_bytes: Index => 3,
    median_scratch_bytes: Index => 4,
    output_bytes: Index => 5,
    text_bytes: Index => 6,
});

/* === Public Functions === */

/* === Private Functions === */
pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "storage frame",
        reason,
    }
}
fn check_agg(agg: Agg, kind: ScalarKind) -> Result<()> {
    let Agg::Of(statistic) = agg else {
        return Ok(());
    };
    if !statistic.answerable() {
        return Err(invalid("percentile rank exceeds 100"));
    }
    if !matches!(
        statistic,
        Aggregate::Count | Aggregate::Min | Aggregate::Max
    ) && !matches!(
        kind,
        ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Float | ScalarKind::Exact
    ) {
        return Err(invalid("numeric aggregate requires numeric expression"));
    }
    Ok(())
}
fn check_output(value: &ScalarValue, agg: Agg) -> Result<()> {
    value.check()?;
    let valid = match agg {
        Agg::Len | Agg::Of(Aggregate::Count) => matches!(value, ScalarValue::Unsigned(_)),
        Agg::Of(Aggregate::Sum | Aggregate::Mean | Aggregate::Median) => matches!(
            value,
            ScalarValue::Missing
                | ScalarValue::Signed(_)
                | ScalarValue::Unsigned(_)
                | ScalarValue::Float(_)
                | ScalarValue::Exact(_)
        ),
        Agg::Of(Aggregate::Rms | Aggregate::Std | Aggregate::Percentile(_)) => {
            matches!(value, ScalarValue::Missing | ScalarValue::Float(_))
        }
        Agg::Last | Agg::Of(Aggregate::Min | Aggregate::Max) => true,
    };
    if !valid {
        return Err(invalid("aggregate output type differs"));
    }
    Ok(())
}
pub(crate) fn heap<T: MemorySize>(value: &T) -> usize {
    value.memory_size().saturating_sub(std::mem::size_of::<T>())
}
pub(crate) fn compare_keys(a: &[ScalarValue], b: &[ScalarValue]) -> Result<Ordering> {
    compare_iter(a.iter(), b.iter())
}
pub(crate) fn compare_iter<'a, 'b>(
    a: impl Iterator<Item = &'a ScalarValue>,
    b: impl Iterator<Item = &'b ScalarValue>,
) -> Result<Ordering> {
    for (a, b) in a.zip(b) {
        let order = a.compare(b)?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

/* === Tests === */
