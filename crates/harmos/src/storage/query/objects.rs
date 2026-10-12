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
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{
    FollowQuery, Query, QueryOptions, QueryResult, decoded_limit, remaining_budget, sealed,
};
use crate::object_catalog::{LOOKUP_MEMORY_BYTES, ObjectDefinition};
use crate::storage::{
    ObjectEntry, ObjectKey, ObjectVersions, QueryFollower, QueryLimits, ReadHistory, ReadKind,
    ReadLimits, ReadSelection, SchemaKey, StorageManager, StorageView,
};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct EncodedObjectQuery {
    pub schema: Option<SchemaKey>,
    pub item: Option<ObjectKey>,
    pub versions: ObjectVersions,
    pub history: ReadHistory,
    pub options: QueryOptions,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for EncodedObjectQuery {
    fn default() -> Self {
        Self {
            schema: None,
            item: None,
            versions: ObjectVersions::Latest,
            history: ReadHistory::Retained,
            options: QueryOptions::default(),
        }
    }
}
impl EncodedObjectQuery {
    fn selection(self) -> ReadSelection {
        let mut selection = self.options.selection(ReadKind::Objects);
        selection.schema = self.schema;
        selection.object_key = self.item;
        selection.object_versions = self.versions;
        selection.history = self.history;
        selection
    }
    fn check<C>(&self, manager: &StorageManager<C>, limits: QueryLimits) -> Result<()> {
        manager
            .inner
            .primary
            .validate_object_versions(self.versions)?;
        if let Some(schema) = &self.schema {
            if LOOKUP_MEMORY_BYTES > limits.memory_bytes.get() {
                return Err(Error::ResourceExhausted {
                    resource: "object query catalog lookup",
                });
            }
            let _claim = manager.reserve_read_now(LOOKUP_MEMORY_BYTES, 1)?;
            (manager.inner.object_definition)(schema)?;
        }
        Ok(())
    }
}
impl sealed::Supported for EncodedObjectQuery {}
impl<C> Query<C> for EncodedObjectQuery {
    type Reader = QueryResult<ObjectEntry, ObjectEntry>;
    fn options(&self) -> &QueryOptions {
        &self.options
    }
    fn preflight(&self, manager: &StorageManager<C>, limits: QueryLimits) -> Result<()> {
        self.check(manager, limits)
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
        self.check(manager, manager.query_execution(&self.options)?)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let decoded_bytes = manager.query_decoded_bytes(&self.options)?;
        let definition = manager.inner.object_definition;
        let selection = self.selection();
        computation.finish(QueryResult {
            reader: view.read_objects(selection)?,
            retained_view: None,
            pending: None,
            page,
            decoded_bytes,
            decode: Box::new(move |entry, allowance| {
                verify(&entry, definition, page.bytes, allowance)?;
                Ok(entry)
            }),
        })
    }
}
impl<C> FollowQuery<C> for EncodedObjectQuery {
    type Raw = ObjectEntry;
    type Value = ObjectEntry;
    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<QueryFollower<ObjectEntry, ObjectEntry>> {
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        self.check(manager, execution)?;
        let definition = manager.inner.object_definition;
        let mode = if replay {
            super::super::follow::Mode::Replay
        } else {
            super::super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<ObjectEntry>(self.selection(), mode, execution)
            .await?;
        let decode = Box::new(move |entry: &ObjectEntry, allowance| {
            verify(entry, definition, page.bytes, allowance)?;
            Ok(entry.clone())
        });
        Ok(QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn verify(
    entry: &ObjectEntry,
    definition: fn(&SchemaKey) -> Result<ObjectDefinition>,
    payload: NonZeroUsize,
    allowance: NonZeroUsize,
) -> Result<()> {
    let scratch = allowance
        .get()
        .checked_sub(entry.memory_size())
        .ok_or_else(decoded_limit)?;
    if scratch < LOOKUP_MEMORY_BYTES {
        return Err(decoded_limit());
    }
    definition(&entry.value.schema)?.validate_admitted(&entry.value, payload, scratch)
}

/* === Tests === */
