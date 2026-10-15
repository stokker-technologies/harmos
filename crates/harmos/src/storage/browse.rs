/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod encoded;
mod fields;

/* === Re-Exports === */
pub use encoded::ObjectFieldsQuery;
pub use fields::{ObjectColumn, ObjectOrder, ObjectPredicate};

/* === Imports === */
// Standard Library Imports
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::manager::{MemoryReservation, encoded_size};
use super::query::{ActiveBudget, execution_timeout, remaining_budget, sealed};
use super::*;
use crate::catalog::Contains;
use crate::storage::catalog::{object_key, object_schema};
use crate::storage::object_fields::{MAX_COLUMNS, MAX_SELECTOR_BYTES};
use crate::{Error, Identifier, MemorySize, Result, schema};
use fields::{check_predicate, check_value, compare_scalar, descriptor, matches_predicate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBrowseRequest {
    pub selection: ReadSelection,
    pub columns: Vec<ObjectColumn>,
    pub projection: Vec<usize>,
    pub predicates: Vec<ObjectPredicate>,
    pub order: Vec<ObjectOrder>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBrowseCursor {
    view: Identifier,
    request: ObjectBrowseRequest,
    last: ObjectMetadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBrowsePage {
    pub entries: Vec<ObjectMetadata>,
    pub next: Option<ObjectBrowseCursor>,
}

pub struct ObjectBrowseQuery<T: StoredObject> {
    pub options: QueryOptions,
    request: ObjectBrowseRequest,
    object: PhantomData<fn() -> T>,
}

pub struct ObjectBrowseResult {
    request: Option<ObjectBrowseRequest>,
    cursor: Option<ObjectBrowseCursor>,
    view: Option<StorageView>,
    reservation: Option<MemoryReservation>,
    page: ReadLimits,
    remaining: Duration,
    decoded_bytes: std::num::NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T: StoredObject> ObjectQuery<'_, T> {
    pub fn project(self, fields: Vec<Identifier>) -> Result<ObjectBrowseQuery<T>> {
        if fields.len() > MAX_COLUMNS {
            return Err(invalid("too many projection fields"));
        }
        let mut selection = self.options.selection(ReadKind::Objects);
        selection.schema = Some(object_schema::<T>()?);
        selection.object_key = self
            .item
            .map(|key| {
                object_key(
                    key,
                    std::num::NonZeroUsize::new(MAX_SELECTOR_BYTES).unwrap(),
                )
            })
            .transpose()?;
        selection.object_versions = self.versions;
        selection.history = self.history;
        let mut query = ObjectBrowseQuery {
            request: ObjectBrowseRequest {
                selection,
                columns: Vec::new(),
                projection: Vec::new(),
                predicates: Vec::new(),
                order: Vec::new(),
            },
            options: self.options,
            object: PhantomData,
        };
        for field in fields {
            let column = query.column(&field)?;
            if query.request.projection.contains(&column) {
                return Err(invalid("duplicate projection field"));
            }
            query.request.projection.push(column);
        }
        Ok(query)
    }
}

impl<T: StoredObject> ObjectBrowseQuery<T> {
    pub fn filter(
        &mut self,
        field: Identifier,
        comparison: ObjectComparison,
        value: ObjectScalar,
    ) -> Result<()> {
        if self.request.predicates.len() == MAX_COLUMNS {
            return Err(invalid("too many predicates"));
        }
        let descriptor = column(T::FIELDS, &field)?;
        check_predicate(&descriptor, comparison, &value)?;
        let column = self.column(&field)?;
        self.request.predicates.push(ObjectPredicate {
            column,
            comparison,
            value,
        });
        Ok(())
    }

    pub fn order_by(
        &mut self,
        field: Identifier,
        direction: ReadOrder,
        nulls: ObjectNullOrder,
    ) -> Result<()> {
        if self.request.order.len() == MAX_COLUMNS {
            return Err(invalid("too many sort fields"));
        }
        if self
            .request
            .order
            .iter()
            .any(|sort| self.request.columns[sort.column].name == field)
        {
            return Err(invalid("duplicate sort field"));
        }
        let column = self.column(&field)?;
        self.request.order.push(ObjectOrder {
            column,
            direction,
            nulls,
        });
        Ok(())
    }

    pub fn request(&self) -> &ObjectBrowseRequest {
        &self.request
    }

    fn column(&mut self, field: &Identifier) -> Result<usize> {
        self.request.column(T::FIELDS, field)
    }
}

impl ObjectBrowseRequest {
    fn column<R>(
        &mut self,
        fields: &[schema::graph::Field<R>],
        field: &Identifier,
    ) -> Result<usize> {
        if let Some(index) = self.columns.iter().position(|column| column.name == *field) {
            return Ok(index);
        }
        if self.columns.len() == MAX_COLUMNS {
            return Err(invalid("too many query columns"));
        }
        let descriptor = column(fields, field)?;
        let index = self.columns.len();
        self.columns.push(descriptor);
        Ok(index)
    }

    pub fn check(&self) -> Result<()> {
        let selection = &self.selection;
        if selection.kind != ReadKind::Objects
            || selection.schema.is_none()
            || selection.scope.is_some()
            || selection.stream.is_some()
            || selection.item.is_some()
            || selection.source_time.is_some()
            || selection.history != ReadHistory::Retained
        {
            return Err(invalid("requires a retained typed object selection"));
        }
        if self.columns.len() > MAX_COLUMNS
            || self.predicates.len() > MAX_COLUMNS
            || self.projection.len() > MAX_COLUMNS
            || self.order.len() > MAX_COLUMNS
        {
            return Err(invalid("query exceeds structural bounds"));
        }
        if selection
            .object_key
            .as_ref()
            .is_some_and(|key| key.as_str().len() > MAX_SELECTOR_BYTES)
        {
            return Err(invalid("object key exceeds query bounds"));
        }
        if selection.schema.as_ref().unwrap().id.as_str().len() > MAX_SELECTOR_BYTES {
            return Err(invalid("schema identity exceeds query bounds"));
        }
        if !self.order.is_empty() && selection.order != ReadOrder::Ascending {
            return Err(invalid(
                "custom ordering requires default admission ordering",
            ));
        }
        for (index, column) in self.columns.iter().enumerate() {
            if column.name.as_str().len() > 256
                || column.tag == 0
                || column.tag >= (1 << 29)
                || (19_000..20_000).contains(&column.tag)
                || self.columns[..index]
                    .iter()
                    .any(|other| other.name == column.name || other.tag == column.tag)
            {
                return Err(invalid("invalid or duplicate scalar descriptor"));
            }
            if !self.projection.contains(&index)
                && !self.predicates.iter().any(|p| p.column == index)
                && !self.order.iter().any(|sort| sort.column == index)
            {
                return Err(invalid("unused query column"));
            }
        }
        for (index, projected) in self.projection.iter().enumerate() {
            if *projected >= self.columns.len() || self.projection[..index].contains(projected) {
                return Err(invalid("invalid projection index"));
            }
        }
        for predicate in &self.predicates {
            let column = self
                .columns
                .get(predicate.column)
                .ok_or(invalid("invalid predicate column"))?;
            check_predicate(column, predicate.comparison, &predicate.value)?;
        }
        for (index, order) in self.order.iter().enumerate() {
            if order.column >= self.columns.len()
                || self.order[..index].iter().any(|p| p.column == order.column)
            {
                return Err(invalid("invalid sort column"));
            }
        }
        Ok(())
    }

    pub fn check_schema<R>(&self, fields: &[schema::graph::Field<R>]) -> Result<()> {
        self.check()?;
        for expected in &self.columns {
            let field = fields
                .iter()
                .find(|field| field.name == expected.name.as_str())
                .ok_or(invalid("unknown persisted field"))?;
            if descriptor(field)? != *expected {
                return Err(invalid("persisted field descriptor mismatch"));
            }
        }
        Ok(())
    }

    pub fn check_view(&self, view: &ReadView, cursor: Option<&ObjectBrowseCursor>) -> Result<()> {
        self.check()?;
        if view.execution_limits().is_none() {
            return Err(invalid("requires bounded query view"));
        }
        view.check_kind(ReadKind::Objects, &self.selection, None)?;
        if let Some(cursor) = cursor {
            if cursor.view != *view.id() || cursor.request != *self {
                return Err(invalid("cursor belongs to a different view or plan"));
            }
            self.check_row(view, &cursor.last)?;
        }
        Ok(())
    }

    pub fn check_row(&self, view: &ReadView, row: &ObjectMetadata) -> Result<()> {
        let through = self.selection.through.unwrap_or(view.saved().sequence);
        if row.schema
            != *self
                .selection
                .schema
                .as_ref()
                .ok_or(invalid("missing schema"))?
            || row.position.sequence <= self.selection.after
            || row.position.sequence > through
            || self
                .selection
                .object_key
                .as_ref()
                .is_some_and(|key| key != &row.item)
            || row.values.len() != self.columns.len()
        {
            return Err(invalid("metadata row does not match selection"));
        }
        view.saved().history.check(&row.position.history)?;
        row.version.check_position(&row.position)?;
        for (column, value) in self.columns.iter().zip(&row.values) {
            check_value(column, value)?;
        }
        for predicate in &self.predicates {
            let value = row
                .values
                .get(predicate.column)
                .ok_or(invalid("missing predicate value"))?;
            if !matches_predicate(value, predicate.comparison, &predicate.value)? {
                return Err(invalid("backend returned a filtered-out object"));
            }
        }
        Ok(())
    }

    pub fn compare(&self, left: &ObjectMetadata, right: &ObjectMetadata) -> Result<Ordering> {
        if self.order.is_empty() {
            let order = left.position.sequence.cmp(&right.position.sequence);
            return Ok(if self.selection.order == ReadOrder::Descending {
                order.reverse()
            } else {
                order
            });
        }
        for order in &self.order {
            let left = left
                .values
                .get(order.column)
                .ok_or(invalid("missing sort value"))?;
            let right = right
                .values
                .get(order.column)
                .ok_or(invalid("missing sort value"))?;
            let ordering = match (left, right) {
                (ObjectScalar::Null, ObjectScalar::Null) => Ordering::Equal,
                (ObjectScalar::Null, _) => {
                    if order.nulls == ObjectNullOrder::First {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
                (_, ObjectScalar::Null) => {
                    if order.nulls == ObjectNullOrder::First {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                }
                _ => {
                    let compared = compare_scalar(left, right)?;
                    if order.direction == ReadOrder::Descending {
                        compared.reverse()
                    } else {
                        compared
                    }
                }
            };
            if ordering != Ordering::Equal {
                return Ok(ordering);
            }
        }
        Ok(left
            .item
            .cmp(&right.item)
            .then_with(|| left.position.sequence.cmp(&right.position.sequence)))
    }
}

impl ObjectBrowseCursor {
    pub fn new(
        view: &ReadView,
        request: ObjectBrowseRequest,
        last: ObjectMetadata,
    ) -> Result<Self> {
        request.check_view(view, None)?;
        request.check_row(view, &last)?;
        Ok(Self {
            view: view.id().clone(),
            request,
            last,
        })
    }

    pub fn last(&self) -> &ObjectMetadata {
        &self.last
    }
}

impl ObjectBrowsePage {
    pub fn check(
        &self,
        view: &ReadView,
        request: &ObjectBrowseRequest,
        cursor: Option<&ObjectBrowseCursor>,
        limits: ReadLimits,
    ) -> Result<()> {
        request.check_view(view, cursor)?;
        limits.check(self.entries.len(), encoded_size(self)?)?;
        if self.memory_size() > output_memory(limits)? {
            return Err(invalid("metadata page exceeds resident bound"));
        }
        let mut previous = cursor.map(ObjectBrowseCursor::last);
        for row in &self.entries {
            request.check_row(view, row)?;
            if let Some(previous) = previous
                && request.compare(previous, row)? != Ordering::Less
            {
                return Err(invalid("unordered metadata page"));
            }
            previous = Some(row);
        }
        if let Some(next) = &self.next {
            request.check_view(view, Some(next))?;
            if self.entries.last() != Some(next.last()) {
                return Err(invalid("continuation must name last returned object"));
            }
        }
        Ok(())
    }
}

impl<T: StoredObject> sealed::Supported for ObjectBrowseQuery<T> {
    fn options_mut(&mut self) -> &mut QueryOptions {
        &mut self.options
    }
}

impl<C: ObjectCatalog + Contains<T>, T: StoredObject> Query<C> for ObjectBrowseQuery<T> {
    type Reader = ObjectBrowseResult;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn preflight(&self, manager: &StorageManager<C>, limits: QueryLimits) -> Result<()> {
        let _reservation = manager.reserve_read_now(self.request.memory_size(), 1)?;
        let mut request = self.request.clone();
        apply_options(&mut request, &self.options);
        request.check_schema(T::FIELDS)?;
        if request.memory_size() > limits.memory_bytes.get() {
            return Err(invalid("query plan exceeds memory bound"));
        }
        manager
            .inner
            .primary
            .validate_object_browse(&request, limits)
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        open_request(self.request, self.options, view, page, remaining)
    }
}

impl ObjectBrowseResult {
    pub fn view(&self) -> Option<&StorageView> {
        self.view.as_ref()
    }

    pub async fn next_page(&mut self) -> Result<Option<QueryPage<ObjectMetadata>>> {
        let Some(view) = &self.view else {
            return Ok(None);
        };
        if self.remaining.is_zero() {
            return Err(execution_timeout());
        }
        let computation = view
            .computation_with(Some(self.remaining), Duration::ZERO)?
            .ok_or(Error::ViewExpired)?;
        let _live = view.pin()?;
        let output = view.reserve_read(self.decoded_bytes.get(), 1)?;
        let timeout = computation.timeout();
        let started = tokio::time::Instant::now();
        let _active = ActiveBudget {
            remaining: &mut self.remaining,
            started,
        };
        let request = self
            .request
            .as_ref()
            .ok_or(invalid("missing browse plan"))?;
        let (mut page, _input) = tokio::time::timeout(timeout, async {
            let _activity = view.activity()?;
            let manager = view.manager();
            let _lifecycle = manager.inner.lifecycle.lock().await;
            let reservation = view.reserve_read(output_memory(self.page)?, self.page.rows.get())?;
            let (_pin, read_view) = view.pin()?;
            request.check_view(&read_view, self.cursor.as_ref())?;
            let page = manager
                .inner
                .bounded(manager.inner.primary.read_object_browse(
                    &read_view,
                    request,
                    self.cursor.as_ref(),
                    self.page,
                ))
                .await?;
            page.check(&read_view, request, self.cursor.as_ref(), self.page)?;
            let _still_live = view.pin()?;
            if started.elapsed() >= timeout {
                return Err(execution_timeout());
            }
            Ok((page, reservation))
        })
        .await
        .map_err(|_| execution_timeout())??;
        // Move selected values without cloning their owned strings. Reservation
        // covers both vectors during projection; the cursor retains all sort inputs.
        for row in &mut page.entries {
            let values = request
                .projection
                .iter()
                .map(|index| std::mem::replace(&mut row.values[*index], ObjectScalar::Null))
                .collect();
            row.values = values;
        }
        if started.elapsed() >= timeout {
            return Err(execution_timeout());
        }
        let output = computation.finish(QueryPage::retain(page.entries, output)?)?;
        self.cursor = page.next;
        if self.cursor.is_none() {
            self.request = None;
            self.reservation = None;
            self.view = None;
        }
        if output.is_empty() {
            return Ok(None);
        }
        Ok(Some(output))
    }
}

impl MemorySize for ObjectBrowseRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.selection))
            .saturating_add(heap(&self.columns))
            .saturating_add(
                self.projection
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(heap(&self.predicates))
            .saturating_add(heap(&self.order))
    }
}

impl MemorySize for ObjectBrowseCursor {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.view))
            .saturating_add(heap(&self.request))
            .saturating_add(heap(&self.last))
    }
}

impl MemorySize for ObjectBrowsePage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.entries))
            .saturating_add(heap(&self.next))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn open_request(
    mut request: ObjectBrowseRequest,
    options: QueryOptions,
    view: StorageView,
    page: ReadLimits,
    remaining: Duration,
) -> Result<ObjectBrowseResult> {
    let started = tokio::time::Instant::now();
    let manager = view.manager();
    manager.check_query_open(&view, &options, page, remaining)?;
    apply_options(&mut request, &options);
    let (_pin, read_view) = view.pin()?;
    request.check_view(&read_view, None)?;
    manager.inner.primary.validate_object_browse(
        &request,
        read_view
            .execution_limits()
            .ok_or(invalid("missing execution limits"))?,
    )?;
    let bytes = request
        .memory_size()
        .checked_add(
            output_memory(page)?
                .checked_mul(2)
                .ok_or(invalid("projection size overflow"))?,
        )
        .ok_or(invalid("plan size overflow"))?;
    let reservation = view.reserve_read(bytes, 1)?;
    let decoded_bytes = manager.query_decoded_bytes(&options)?;
    Ok(ObjectBrowseResult {
        request: Some(request),
        cursor: None,
        view: Some(view),
        reservation: Some(reservation),
        page,
        remaining: remaining_budget(remaining, started)?,
        decoded_bytes,
    })
}

fn column<R>(fields: &[schema::graph::Field<R>], name: &Identifier) -> Result<ObjectColumn> {
    let field = fields
        .iter()
        .find(|field| field.name == name.as_str())
        .ok_or(invalid("unknown object field"))?;
    descriptor(field)
}

fn apply_options(request: &mut ObjectBrowseRequest, options: &QueryOptions) {
    request.selection.after = options.after;
    request.selection.through = options.through;
    request.selection.order = options.order;
}

fn heap<T: MemorySize>(value: &T) -> usize {
    value.memory_size().saturating_sub(std::mem::size_of::<T>())
}

fn output_memory(page: ReadLimits) -> Result<usize> {
    page.bytes
        .get()
        .checked_mul(4)
        .ok_or(invalid("output size overflow"))
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "object browse",
        reason,
    }
}

/* === Tests === */
