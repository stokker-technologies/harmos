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
use std::borrow::Cow;
use std::cmp::Ordering;
use std::future::Future;
use std::num::NonZeroU128;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::super::reduce::{ExactNumber, Scratch};
use super::*;

/* === Constants === */
// Cancellation Is Polled On Every Page And At Least This Often Between Rows.
const POLL_ROWS: usize = 256;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedRow {
    pub capture_order: u64,
    pub values: Vec<ScalarValue>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanCursor {
    pub token: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScanPage {
    pub rows: Vec<ProjectedRow>,
    pub next: Option<ScanCursor>,
}
pub(super) struct Pass {
    pub(super) source: usize,
    pub(super) order: ScanOrder,
    pub(super) joins: Vec<usize>,
    pub(super) group_bys: Vec<usize>,
}
pub(super) struct Table {
    bound: usize,
    groups: Vec<Group>,
    // Group Indices Sorted By Key For Fallible Binary Search.
    sorted: Vec<usize>,
    members: usize,
    scratch: Vec<Scratch>,
}
pub(super) struct Group {
    key: Vec<ScalarValue>,
    aggs: Vec<Accumulator>,
    members: Vec<Member>,
}
pub(super) struct Member {
    value: ScalarValue,
    order: ScalarValue,
    capture: u64,
    aggs: Vec<Accumulator>,
}
struct Stream {
    scan: usize,
    order: ScanOrder,
    rows: Vec<ProjectedRow>,
    position: usize,
    next: Option<ScanCursor>,
    started: bool,
    seen: usize,
    last: Option<ProjectedRow>,
}
struct Join<'f> {
    index: usize,
    as_of: &'f AsOf,
    right: Option<Stream>,
    candidate: Candidate,
    hit: bool,
    consumed: usize,
}
struct Ledger {
    remaining: usize,
}
// Everything A Pass Needs To Pull Pages: Source, Limits, Text Ledger And Polling.
struct Pull<'p, S, F> {
    frame: &'p Frame,
    source: &'p mut S,
    limits: ReadLimits,
    ledger: Ledger,
    check: &'p mut F,
}
struct Context<'r, 'f> {
    row: &'r ProjectedRow,
    joins: &'r [Join<'f>],
    left: &'r Stream,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanOrder {
    Capture,
    Keyed { by: Vec<usize>, time: Option<usize> },
}
enum Candidate {
    None,
    Page(usize),
    Owned(ProjectedRow),
}

/* === Traits === */
pub trait ScanSource {
    fn page(
        &mut self,
        scan: usize,
        order: &ScanOrder,
        cursor: Option<&ScanCursor>,
        limits: ReadLimits,
    ) -> impl Future<Output = Result<ScanPage>> + Send;
}

/* === Implementations === */
impl ScanOrder {
    pub fn compare(&self, a: &ProjectedRow, b: &ProjectedRow) -> Result<Ordering> {
        let Self::Keyed { by, time } = self else {
            return Ok(a.capture_order.cmp(&b.capture_order));
        };
        for &column in by {
            let order = a.values[column].compare(&b.values[column])?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        if let Some(column) = time {
            let order = a.values[*column].compare(&b.values[*column])?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(a.capture_order.cmp(&b.capture_order))
    }
}
impl MemorySize for ProjectedRow {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(heap(&self.values))
    }
}
impl Ledger {
    fn adjust(&mut self, before: usize, after: usize) -> Result<()> {
        if after <= before {
            self.remaining = self.remaining.saturating_add(before - after);
            return Ok(());
        }
        self.remaining = self
            .remaining
            .checked_sub(after - before)
            .ok_or(invalid("retained text exceeds explained allowance"))?;
        Ok(())
    }
}
impl Stream {
    fn new(scan: usize, order: ScanOrder) -> Self {
        Self {
            scan,
            order,
            rows: Vec::new(),
            position: 0,
            next: None,
            started: false,
            seen: 0,
            last: None,
        }
    }

    fn needs_page(&self) -> bool {
        self.position == self.rows.len() && (!self.started || self.next.is_some())
    }

    async fn peek<S: ScanSource, F: FnMut() -> Result<()>>(
        &mut self,
        pull: &mut Pull<'_, S, F>,
    ) -> Result<Option<&ProjectedRow>> {
        while self.needs_page() {
            (pull.check)()?;
            let page = pull
                .source
                .page(self.scan, &self.order, self.next.as_ref(), pull.limits)
                .await?;
            (pull.check)()?;
            self.admit(pull.frame, &page, pull.limits)?;

            // Keep A Clone Of The Last Row So The Next Page Proves Its Order.
            if let Some(last) = page.rows.last() {
                let before = self.last.as_ref().map_or(0, text_bytes);
                pull.ledger.adjust(before, text_bytes(last))?;
                self.last = Some(clone_row(last)?);
            }
            self.started = true;
            self.next = page.next;
            self.rows = page.rows;
            self.position = 0;
        }
        Ok(self.rows.get(self.position))
    }

    fn admit(&mut self, frame: &Frame, page: &ScanPage, limits: ReadLimits) -> Result<()> {
        let columns = &frame.scans[self.scan].columns;
        if page.rows.is_empty() && page.next.is_some() {
            return Err(invalid("scan returned an empty intermediate page"));
        }
        let mut bytes = page.next.as_ref().map_or(0, |cursor| cursor.token.len());
        for (index, row) in page.rows.iter().enumerate() {
            if row.values.len() != columns.len() {
                return Err(invalid("scan row width differs from columns"));
            }
            for (value, column) in row.values.iter().zip(columns) {
                value.check()?;
                if value.kind().is_some_and(|kind| kind != column.kind) {
                    return Err(invalid("scan row differs from column type"));
                }
            }
            bytes = bytes.saturating_add(text_bytes(row));
            let previous = match index {
                0 => self.last.as_ref(),
                _ => Some(&page.rows[index - 1]),
            };
            if let Some(previous) = previous
                && !self.order.compare(previous, row)?.is_lt()
            {
                return Err(invalid("scan rows are not in the requested order"));
            }
        }
        limits.check(page.rows.len(), bytes)?;
        self.seen = self
            .seen
            .checked_add(page.rows.len())
            .filter(|&seen| seen <= frame.max_rows.get())
            .ok_or(invalid("scan rows exceed bound"))?;
        Ok(())
    }
}
impl<'f> Join<'f> {
    fn reuses_left(&self) -> bool {
        self.right.is_none()
    }

    fn candidate<'s>(&'s self, left: &'s Stream) -> Option<&'s ProjectedRow> {
        let stream = self.right.as_ref().unwrap_or(left);
        match &self.candidate {
            Candidate::None => None,
            Candidate::Page(index) => stream.rows.get(*index),
            Candidate::Owned(row) => Some(row),
        }
    }

    // A Candidate Borrowed From A Page Must Outlive That Page's Replacement.
    fn own_candidate(&mut self, left: &Stream, ledger: &mut Ledger) -> Result<()> {
        let Candidate::Page(index) = self.candidate else {
            return Ok(());
        };
        let stream = self.right.as_ref().unwrap_or(left);
        let row = &stream.rows[index];
        ledger.adjust(0, text_bytes(row))?;
        self.candidate = Candidate::Owned(clone_row(row)?);
        Ok(())
    }

    fn replace_candidate(&mut self, index: usize, ledger: &mut Ledger) -> Result<()> {
        if let Candidate::Owned(row) = &self.candidate {
            ledger.adjust(text_bytes(row), 0)?;
        }
        self.candidate = Candidate::Page(index);
        Ok(())
    }

    fn eligible_left(&self, row: &ProjectedRow) -> bool {
        matches!(row.values[self.as_of.on.0], ScalarValue::Datetime(_))
            && self
                .as_of
                .by
                .iter()
                .all(|&(key, _)| !absent(&row.values[key]))
    }

    fn eligible_right(&self, row: &ProjectedRow) -> Result<bool> {
        let as_of = self.as_of;
        if !matches!(row.values[as_of.on.1], ScalarValue::Datetime(_))
            || as_of.by.iter().any(|&(_, key)| absent(&row.values[key]))
        {
            return Ok(false);
        }
        if let AsOfStrategy::Backward { present } = as_of.strategy
            && matches!(row.values[present], ScalarValue::Missing)
        {
            return Ok(false);
        }
        for predicate in &as_of.right_filter {
            let Expr::Col(column) = predicate.expr else {
                return Err(invalid("right filter requires source column"));
            };
            if !satisfies(&row.values[column], predicate)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn keys(&self, right: &ProjectedRow, left: &ProjectedRow) -> Result<Ordering> {
        for &(left_key, right_key) in &self.as_of.by {
            let order = right.values[right_key].compare(&left.values[left_key])?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(Ordering::Equal)
    }

    // Right Rows Strictly Before The Left Row In The Merge Order Are Consumed.
    fn precedes(&self, right: &ProjectedRow, left: &ProjectedRow) -> Result<bool> {
        let keys = self.keys(right, left)?;
        if !keys.is_eq() {
            return Ok(keys.is_lt());
        }
        Ok(match self.as_of.strategy {
            AsOfStrategy::Backward { .. } => !right.values[self.as_of.on.1]
                .compare(&left.values[self.as_of.on.0])?
                .is_gt(),
            AsOfStrategy::Previous => right.capture_order < left.capture_order,
        })
    }

    async fn advance<S: ScanSource, F: FnMut() -> Result<()>>(
        &mut self,
        left: &Stream,
        row: &ProjectedRow,
        pull: &mut Pull<'_, S, F>,
    ) -> Result<()> {
        self.hit = false;
        if !self.eligible_left(row) {
            return Ok(());
        }
        if self.right.is_some() {
            loop {
                if self.right.as_ref().is_some_and(Stream::needs_page) {
                    self.own_candidate(left, &mut pull.ledger)?;
                }
                let right = self.right.as_mut().expect("separate right stream");
                if right.peek(pull).await?.is_none() {
                    break;
                }
                let position = right.position;
                if !self.precedes(self.right_row(position), row)? {
                    break;
                }
                if self.eligible_right(self.right_row(position))? {
                    self.replace_candidate(position, &mut pull.ledger)?;
                }
                self.right.as_mut().expect("separate right stream").position += 1;
                self.consumed += 1;
                if self.consumed.is_multiple_of(POLL_ROWS) {
                    (pull.check)()?;
                }
            }
        }
        self.hit = match self.candidate(left) {
            Some(candidate) => self.keys(candidate, row)?.is_eq(),
            None => false,
        };
        Ok(())
    }

    fn right_row(&self, position: usize) -> &ProjectedRow {
        &self.right.as_ref().expect("separate right stream").rows[position]
    }

    // A Shift Observes Each Left Row After Matching It, As The Next Row's Candidate.
    fn observe(&mut self, left: &Stream, row: &ProjectedRow, ledger: &mut Ledger) -> Result<()> {
        if self.reuses_left() && self.eligible_right(row)? {
            self.replace_candidate(left.position, ledger)?;
        }
        Ok(())
    }
}
impl Context<'_, '_> {
    fn join(&self, join: usize) -> Option<(&Join<'_>, &ProjectedRow)> {
        let join = self
            .joins
            .iter()
            .find(|candidate| candidate.index == join)?;
        if !join.hit {
            return None;
        }
        Some((join, join.candidate(self.left)?))
    }

    fn eval(&self, expr: &Expr) -> Result<Cow<'_, ScalarValue>> {
        Ok(match *expr {
            Expr::Col(column) => Cow::Borrowed(&self.row.values[column]),
            Expr::AsOf { join, col } => match self.join(join) {
                Some((_, matched)) => Cow::Borrowed(&matched.values[col]),
                None => Cow::Owned(ScalarValue::Missing),
            },
            Expr::Elapsed(join) => {
                let Some((join, matched)) = self.join(join) else {
                    return Ok(Cow::Owned(ScalarValue::Missing));
                };
                let (ScalarValue::Datetime(end), ScalarValue::Datetime(start)) = (
                    &self.row.values[join.as_of.on.0],
                    &matched.values[join.as_of.on.1],
                ) else {
                    return Err(invalid("elapsed requires datetimes"));
                };
                Cow::Owned(elapsed(*start, *end)?)
            }
        })
    }

    fn selected(&self, filter: &[Predicate]) -> Result<bool> {
        for predicate in filter {
            let value = self.eval(&predicate.expr)?;
            if !satisfies(value.as_ref(), predicate)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl Table {
    fn new(frame: &Frame, group_by: &GroupBy, ledger: &mut Ledger) -> Result<Self> {
        let bound = group_by.group_bound(frame.max_groups.get());
        let accumulators = match group_by.last_by {
            Some(_) => frame.max_groups.get(),
            None => bound,
        };
        let mut scratch = slots(group_by.aggs.len())?;
        for _ in &group_by.aggs {
            let slots = median_slots(frame.max_rows.get(), accumulators)
                .ok_or(invalid("median scratch overflows"))?;
            scratch.push(Scratch::new(slots));
        }
        let mut table = Self {
            bound,
            groups: slots(bound)?,
            sorted: slots(bound)?,
            members: 0,
            scratch,
        };

        // Explicit Groups Exist Before Any Row; Ungrouped Input Has One Group.
        match &group_by.groups {
            Some(keys) => {
                for key in keys {
                    let at = table
                        .find(key.iter())?
                        .err()
                        .ok_or(invalid("duplicate explicit group"))?;
                    table.insert(at, key.iter(), group_by, ledger)?;
                }
            }
            None if group_by.keys.is_empty() => {
                table.insert(0, std::iter::empty(), group_by, ledger)?;
            }
            None => {}
        }
        Ok(table)
    }

    fn find<'v>(
        &self,
        key: impl Iterator<Item = &'v ScalarValue> + Clone,
    ) -> Result<std::result::Result<usize, usize>> {
        let mut low = 0;
        let mut high = self.sorted.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match compare_iter(self.groups[self.sorted[middle]].key.iter(), key.clone())? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(self.sorted[middle])),
            }
        }
        Ok(Err(low))
    }

    fn insert<'v>(
        &mut self,
        at: usize,
        key: impl Iterator<Item = &'v ScalarValue>,
        group_by: &GroupBy,
        ledger: &mut Ledger,
    ) -> Result<usize> {
        if self.groups.len() == self.bound {
            return Err(invalid("groups exceed bound"));
        }
        let mut owned = slots(group_by.keys.len())?;
        for value in key {
            ledger.adjust(0, value_text(value))?;
            owned.push(clone_value(value)?);
        }
        let aggs = match group_by.last_by {
            Some(_) => Vec::new(),
            None => accumulators(group_by)?,
        };
        self.groups.push(Group {
            key: owned,
            aggs,
            members: Vec::new(),
        });
        self.sorted.insert(at, self.groups.len() - 1);
        Ok(self.groups.len() - 1)
    }

    fn push(
        &mut self,
        frame: &Frame,
        group_by: &GroupBy,
        context: &Context<'_, '_>,
        ledger: &mut Ledger,
    ) -> Result<()> {
        if !context.selected(&group_by.filter)? {
            return Ok(());
        }
        let row = context.row;
        let key = group_by.keys.iter().map(|&column| &row.values[column]);
        let index = match self.find(key.clone())? {
            Ok(index) => index,
            Err(_) if group_by.groups.is_some() => return Ok(()),
            Err(at) => self.insert(at, key, group_by, ledger)?,
        };

        // Latest-Member Selection Keeps One Sub-State Per Member Value.
        let capture = row.capture_order;
        let aggs = match &group_by.last_by {
            None => &mut self.groups[index].aggs,
            Some(last_by) => {
                let value = &row.values[last_by.member];
                let order = &row.values[last_by.order];
                if absent(value) {
                    return Err(invalid("member identity is absent"));
                }
                let members = &mut self.groups[index].members;
                let mut low = 0;
                let mut high = members.len();
                let mut found = None;
                while low < high {
                    let middle = low + (high - low) / 2;
                    match members[middle].value.compare(value)? {
                        Ordering::Less => low = middle + 1,
                        Ordering::Greater => high = middle,
                        Ordering::Equal => {
                            found = Some(middle);
                            break;
                        }
                    }
                }
                let position = match found {
                    Some(position) => {
                        let member = &mut members[position];
                        let later = order
                            .compare(&member.order)?
                            .then(capture.cmp(&member.capture))
                            .is_gt();
                        if later {
                            ledger.adjust(value_text(&member.order), value_text(order))?;
                            member.order = clone_value(order)?;
                            member.capture = capture;
                        }
                        position
                    }
                    None => {
                        if self.members == frame.max_groups.get() {
                            return Err(invalid("member sub-states exceed group bound"));
                        }
                        members
                            .try_reserve_exact(1)
                            .map_err(|_| invalid("member allocation failed"))?;
                        ledger.adjust(0, value_text(value).saturating_add(value_text(order)))?;
                        members.insert(
                            low,
                            Member {
                                value: clone_value(value)?,
                                order: clone_value(order)?,
                                capture,
                                aggs: accumulators(group_by)?,
                            },
                        );
                        self.members += 1;
                        low
                    }
                };
                &mut members[position].aggs
            }
        };

        for ((accumulator, (expr, _)), scratch) in
            aggs.iter_mut().zip(&group_by.aggs).zip(&mut self.scratch)
        {
            let value = context.eval(expr)?;
            let before = accumulator.text_bytes();
            accumulator.push(capture, &value, scratch)?;
            ledger.adjust(before, accumulator.text_bytes())?;
        }
        Ok(())
    }

    fn finish(
        self,
        group_by: &GroupBy,
        ledger: &mut Ledger,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<GroupOutput> {
        let Self {
            groups, mut sorted, ..
        } = self;
        let mut rows = slots(groups.len())?;
        for group in groups {
            check()?;
            let aggs = match &group_by.last_by {
                None => group.aggs,
                Some(_) => {
                    // The Member With The Greatest (Order, Capture) Wins; Others Drop.
                    let mut latest: Option<Member> = None;
                    for member in group.members {
                        let replace = match &latest {
                            None => true,
                            Some(previous) => member
                                .order
                                .compare(&previous.order)?
                                .then(member.capture.cmp(&previous.capture))
                                .is_gt(),
                        };
                        if replace {
                            latest = Some(member);
                        }
                    }
                    match latest {
                        Some(member) => member.aggs,
                        None => accumulators(group_by)?,
                    }
                }
            };
            let mut values = slots(aggs.len())?;
            for accumulator in aggs {
                values.push(accumulator.finish(check)?);
            }
            rows.push(GroupRow {
                key: group.key,
                values,
            });
        }

        // Implicit Groups Leave In Key Order; Explicit Groups Keep Supplied Order.
        if group_by.groups.is_none() {
            let mut rank = slots(sorted.len())?;
            rank.resize(sorted.len(), 0);
            for (position, &index) in sorted.iter().enumerate() {
                rank[index] = position;
            }
            for index in 0..rows.len() {
                while rank[index] != index {
                    let target = rank[index];
                    rows.swap(index, target);
                    rank.swap(index, target);
                }
            }
            sorted.clear();
        }

        let outer = match &group_by.outer {
            None => None,
            Some(aggs) => {
                let mut values = slots(aggs.len())?;
                for (column, &agg) in aggs.iter().enumerate() {
                    check()?;
                    let mut accumulator = Accumulator::new(agg);
                    let mut scratch = Scratch::new(
                        median_slots(rows.len(), 1).ok_or(invalid("median scratch overflows"))?,
                    );
                    for (position, row) in rows.iter().enumerate() {
                        accumulator.push(position as u64, &row.values[column], &mut scratch)?;
                    }
                    ledger.adjust(0, accumulator.text_bytes())?;
                    values.push(accumulator.finish(check)?);
                }
                Some(values)
            }
        };
        Ok(GroupOutput {
            groups: rows,
            outer,
        })
    }
}

/* === Public Functions === */
pub async fn collect(
    frame: &Frame,
    source: &mut impl ScanSource,
    explain: &Explain,
    page: ReadLimits,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Collected> {
    if frame.explain(page)? != *explain {
        return Err(invalid("explain differs from frame and page"));
    }
    check()?;
    let mut pull = Pull {
        frame,
        source,
        limits: page,
        ledger: Ledger {
            remaining: explain.text_bytes,
        },
        check,
    };
    let mut outputs: Vec<Option<GroupOutput>> = slots(frame.group_bys.len())?;
    outputs.resize_with(frame.group_bys.len(), || None);

    for pass in plan(frame)? {
        let finished = run(&pass, &mut pull).await?;
        for (index, output) in pass.group_bys.iter().zip(finished) {
            outputs[*index] = Some(output);
        }
    }

    let mut collected = Collected {
        outputs: slots(outputs.len())?,
    };
    for output in outputs {
        collected
            .outputs
            .push(output.ok_or(invalid("group-by was not executed"))?);
    }
    (pull.check)()?;
    if collected.memory_size() > explain.output_bytes.saturating_add(explain.text_bytes) {
        return Err(invalid("output exceeds explained capacity"));
    }
    Ok(collected)
}

/* === Private Functions === */
pub(super) fn plan(frame: &Frame) -> Result<Vec<Pass>> {
    let mut passes: Vec<Pass> = Vec::new();
    for (index, group_by) in frame.group_bys.iter().enumerate() {
        let join = group_by.join()?;
        let order = match join {
            None => ScanOrder::Capture,
            Some(join) => left_order(&frame.as_ofs[join]),
        };
        let at = passes
            .iter()
            .position(|pass| pass.source == group_by.source && pass.order == order);
        let pass = match at {
            Some(at) => &mut passes[at],
            None => {
                passes.push(Pass {
                    source: group_by.source,
                    order,
                    joins: Vec::new(),
                    group_bys: Vec::new(),
                });
                passes.last_mut().expect("pass was just pushed")
            }
        };
        pass.group_bys.push(index);
        if let Some(join) = join
            && !pass.joins.contains(&join)
        {
            pass.joins.push(join);
        }
    }
    Ok(passes)
}
pub(super) fn reuses_left(as_of: &AsOf) -> bool {
    as_of.left == as_of.right
        && as_of.strategy == AsOfStrategy::Previous
        && as_of.by.iter().all(|(left, right)| left == right)
}
pub(super) fn median_slots(values: usize, accumulators: usize) -> Option<usize> {
    values
        .checked_mul(2)?
        .checked_add(accumulators.checked_mul(4)?)
}
fn left_order(as_of: &AsOf) -> ScanOrder {
    ScanOrder::Keyed {
        by: as_of.by.iter().map(|&(left, _)| left).collect(),
        time: match as_of.strategy {
            AsOfStrategy::Backward { .. } => Some(as_of.on.0),
            AsOfStrategy::Previous => None,
        },
    }
}
pub(super) fn right_order(as_of: &AsOf) -> ScanOrder {
    ScanOrder::Keyed {
        by: as_of.by.iter().map(|&(_, right)| right).collect(),
        time: match as_of.strategy {
            AsOfStrategy::Backward { .. } => Some(as_of.on.1),
            AsOfStrategy::Previous => None,
        },
    }
}
async fn run<S: ScanSource, F: FnMut() -> Result<()>>(
    pass: &Pass,
    pull: &mut Pull<'_, S, F>,
) -> Result<Vec<GroupOutput>> {
    let frame = pull.frame;
    let mut tables = slots(pass.group_bys.len())?;
    for &index in &pass.group_bys {
        tables.push(Table::new(
            frame,
            &frame.group_bys[index],
            &mut pull.ledger,
        )?);
    }
    let mut joins = slots(pass.joins.len())?;
    for &index in &pass.joins {
        let as_of = &frame.as_ofs[index];
        let right = (!reuses_left(as_of)).then(|| Stream::new(as_of.right, right_order(as_of)));
        joins.push(Join {
            index,
            as_of,
            right,
            candidate: Candidate::None,
            hit: false,
            consumed: 0,
        });
    }

    let mut left = Stream::new(pass.source, pass.order.clone());
    let mut rows = 0usize;
    loop {
        if left.needs_page() {
            for join in joins.iter_mut().filter(|join| join.reuses_left()) {
                join.own_candidate(&left, &mut pull.ledger)?;
            }
        }
        if left.peek(pull).await?.is_none() {
            break;
        }
        rows += 1;
        if rows.is_multiple_of(POLL_ROWS) {
            (pull.check)()?;
        }

        // Merge Every Join Up To This Row, Then Feed Every Bound Group-By.
        let row = &left.rows[left.position];
        for join in &mut joins {
            join.advance(&left, row, pull).await?;
        }
        let context = Context {
            row,
            joins: &joins,
            left: &left,
        };
        for (table, &index) in tables.iter_mut().zip(&pass.group_bys) {
            table.push(frame, &frame.group_bys[index], &context, &mut pull.ledger)?;
        }
        for join in &mut joins {
            join.observe(&left, row, &mut pull.ledger)?;
        }
        left.position += 1;
    }

    // Release Retained Witnesses And Candidates Before The Next Pass.
    let ledger = &mut pull.ledger;
    if let Some(last) = &left.last {
        ledger.adjust(text_bytes(last), 0)?;
    }
    for join in &joins {
        if let Some(last) = join.right.as_ref().and_then(|right| right.last.as_ref()) {
            ledger.adjust(text_bytes(last), 0)?;
        }
        if let Candidate::Owned(row) = &join.candidate {
            ledger.adjust(text_bytes(row), 0)?;
        }
    }

    let mut outputs = slots(tables.len())?;
    for (table, &index) in tables.into_iter().zip(&pass.group_bys) {
        outputs.push(table.finish(&frame.group_bys[index], &mut pull.ledger, pull.check)?);
    }
    Ok(outputs)
}
fn accumulators(group_by: &GroupBy) -> Result<Vec<Accumulator>> {
    let mut aggs = slots(group_by.aggs.len())?;
    for (_, agg) in &group_by.aggs {
        aggs.push(Accumulator::new(*agg));
    }
    Ok(aggs)
}
fn satisfies(value: &ScalarValue, predicate: &Predicate) -> Result<bool> {
    if absent(value) || absent(&predicate.value) {
        return Ok(match predicate.compare {
            Compare::Eq => *value == predicate.value,
            Compare::Ne => *value != predicate.value,
            _ => false,
        });
    }
    let order = value.compare(&predicate.value)?;
    Ok(match predicate.compare {
        Compare::Eq => order.is_eq(),
        Compare::Ne => !order.is_eq(),
        Compare::Lt => order.is_lt(),
        Compare::Le => !order.is_gt(),
        Compare::Gt => order.is_gt(),
        Compare::Ge => !order.is_lt(),
    })
}
fn elapsed(start: i128, end: i128) -> Result<ScalarValue> {
    if end < start {
        return Ok(ScalarValue::Missing);
    }
    let nanos = end
        .checked_sub(start)
        .ok_or(invalid("elapsed time overflow"))? as u128;
    Ok(match u64::try_from(nanos) {
        Ok(nanos) => ScalarValue::Unsigned(nanos),
        Err(_) => ScalarValue::Exact(ExactNumber::new(false, nanos, NonZeroU128::MIN)),
    })
}
fn absent(value: &ScalarValue) -> bool {
    matches!(value, ScalarValue::Missing | ScalarValue::Null)
}
fn value_text(value: &ScalarValue) -> usize {
    match value {
        ScalarValue::Text(text) => text.capacity(),
        _ => 0,
    }
}
fn text_bytes(row: &ProjectedRow) -> usize {
    row.values
        .iter()
        .map(value_text)
        .fold(0, usize::saturating_add)
}
fn clone_value(value: &ScalarValue) -> Result<ScalarValue> {
    Ok(match value {
        ScalarValue::Text(text) => {
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| invalid("text allocation failed"))?;
            owned.push_str(text);
            ScalarValue::Text(owned)
        }
        value => value.clone(),
    })
}
fn clone_row(row: &ProjectedRow) -> Result<ProjectedRow> {
    let mut values = slots(row.values.len())?;
    for value in &row.values {
        values.push(clone_value(value)?);
    }
    Ok(ProjectedRow {
        capture_order: row.capture_order,
        values,
    })
}
fn slots<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| invalid("frame allocation failed"))?;
    Ok(values)
}

/* === Tests === */
