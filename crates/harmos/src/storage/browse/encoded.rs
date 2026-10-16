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
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{
    MAX_COLUMNS, ObjectBrowseRequest, ObjectBrowseResult, ObjectColumn, ObjectOrder,
    ObjectPredicate, apply_options, invalid, open_request,
};
use crate::storage::catalog::LOOKUP_MEMORY_BYTES;
use crate::storage::manager::MemoryReservation;
use crate::storage::query::sealed;
use crate::storage::{
    EncodedObjectQuery, ObjectFields, QueryLimits, QueryOptions, ReadHistory, ReadKind, ReadLimits,
    ReadOrder, ReadSelection, StorageManager, StorageQuery, StorageView,
};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ObjectFieldsQuery {
    pub options: QueryOptions,
    pub fields: ObjectFields,
    selection: ReadSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl EncodedObjectQuery {
    pub fn project(self, fields: ObjectFields) -> Result<ObjectFieldsQuery> {
        fields.validate()?;
        if self.schema.is_none() || self.history != ReadHistory::Retained {
            return Err(invalid(
                "scalar projection requires one schema and retained history",
            ));
        }
        let mut selection = self.options.selection(ReadKind::Objects);
        selection.schema = self.schema;
        selection.object_key = self.item;
        selection.object_versions = self.versions;
        selection.history = self.history;
        Ok(ObjectFieldsQuery {
            options: self.options,
            fields,
            selection,
        })
    }
}
impl ObjectFieldsQuery {
    fn prepare<C>(
        &self,
        manager: &StorageManager<C>,
        limits: QueryLimits,
    ) -> Result<(ObjectBrowseRequest, MemoryReservation)> {
        self.fields.validate()?;
        // Hold One Claim Before Copying Any Selector, Column Or Predicate Text.
        let columns =
            (self.fields.projection.len() + self.fields.filters.len() + self.fields.order.len())
                .min(MAX_COLUMNS);
        let bytes = self
            .fields
            .memory_size()
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(self.selection.memory_size()))
            .and_then(|bytes| {
                bytes.checked_add(
                    columns
                        * (std::mem::size_of::<ObjectColumn>()
                            + 256
                            + 3 * std::mem::size_of::<usize>()),
                )
            })
            .and_then(|bytes| bytes.checked_add(LOOKUP_MEMORY_BYTES))
            .ok_or(invalid("scalar plan memory overflow"))?;
        if bytes > limits.memory_bytes.get() {
            return Err(invalid("query plan exceeds memory bound"));
        }
        let claim = manager.reserve_read_now(bytes, 1)?;
        let schema = self
            .selection
            .schema
            .as_ref()
            .ok_or(invalid("missing projection schema"))?;
        let definition = (manager.inner.object_definition)(schema)?;
        let mut request = ObjectBrowseRequest {
            selection: self.selection.clone(),
            columns: Vec::new(),
            projection: Vec::new(),
            predicates: Vec::new(),
            order: Vec::new(),
        };
        apply_options(&mut request, &self.options);
        for field in &self.fields.projection {
            let column = request.column((definition.declaration().fields)(), field)?;
            if request.projection.contains(&column) {
                return Err(invalid("duplicate projection field"));
            }
            request.projection.push(column);
        }
        for filter in &self.fields.filters {
            let column = request.column((definition.declaration().fields)(), &filter.field)?;
            request.predicates.push(ObjectPredicate {
                column,
                comparison: filter.comparison,
                value: filter.value.clone(),
            });
        }
        for sort in &self.fields.order {
            let column = request.column((definition.declaration().fields)(), &sort.field)?;
            request.order.push(ObjectOrder {
                column,
                direction: if sort.descending {
                    ReadOrder::Descending
                } else {
                    ReadOrder::Ascending
                },
                nulls: sort.nulls,
            });
        }
        request.check_schema((definition.declaration().fields)())?;
        if request.memory_size() > bytes {
            return Err(invalid("scalar plan exceeded its memory claim"));
        }
        Ok((request, claim))
    }
}
impl sealed::Supported for ObjectFieldsQuery {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}
impl<C> StorageQuery<C> for ObjectFieldsQuery {
    type Reader = ObjectBrowseResult;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn preflight(&self, manager: &StorageManager<C>, limits: QueryLimits) -> Result<()> {
        let (request, _claim) = self.prepare(manager, limits)?;
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
        let started = tokio::time::Instant::now();
        let manager = view.manager();
        manager.check_query_open(&view, &self.options, page, remaining)?;
        let execution = manager.query_execution(&self.options)?;
        let (request, _claim) = self.prepare(manager, execution)?;
        let remaining = super::remaining_budget(remaining, started)?;
        open_request(request, self.options, view, page, remaining)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
