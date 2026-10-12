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
pub use crate::storage::read_selection::{
    ReadHistory, ReadKind, ReadLimits, ReadOrder, ReadSelection, SourceTimeRange,
};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::{ObjectVersions, QueryLimits, StorageCursor};
use crate::{Error, Identifier, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendSnapshot {
    pub saved: StorageCursor,
    pub token: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadView {
    id: Identifier,
    accepted: StorageCursor,
    saved: StorageCursor,
    snapshot: BackendSnapshot,
    execution: Option<QueryLimits>,
    computation: Option<super::views::ComputationLink>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadCursor {
    view: Identifier,
    selection: ReadSelection,
    position: StorageCursor,
}

#[derive(Clone, Debug)]
pub struct ReadPage<T> {
    pub entries: Vec<T>,
    pub next: Option<ReadCursor>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl ReadView {
    pub fn retain_computation(&self) -> Result<Option<super::StorageViewComputation>> {
        if self.execution.is_none() {
            return Ok(None);
        }
        self.computation
            .as_ref()
            .map(|link| link.retain())
            .transpose()
    }

    pub(super) fn with_computation(mut self, link: super::views::ComputationLink) -> Self {
        self.computation = Some(link);
        self
    }

    pub fn execution_limits(&self) -> Option<QueryLimits> {
        self.execution
    }

    pub(super) fn with_execution(mut self, limits: Option<QueryLimits>) -> Self {
        self.execution = limits;
        self
    }

    pub fn new(
        id: Identifier,
        accepted: StorageCursor,
        saved: StorageCursor,
        snapshot: BackendSnapshot,
    ) -> Result<Self> {
        if !accepted.covers(&saved)? || saved != snapshot.saved || snapshot.token.is_empty() {
            return Err(Error::InvalidInput {
                field: "read view",
                reason: "inconsistent frontiers or empty snapshot identity",
            });
        }

        Ok(Self {
            id,
            accepted,
            saved,
            snapshot,
            execution: None,
            computation: None,
        })
    }

    pub fn id(&self) -> &Identifier {
        &self.id
    }

    pub fn accepted(&self) -> &StorageCursor {
        &self.accepted
    }

    pub fn saved(&self) -> &StorageCursor {
        &self.saved
    }

    pub fn snapshot(&self) -> &BackendSnapshot {
        &self.snapshot
    }

    pub fn check_kind(
        &self,
        kind: ReadKind,
        selection: &ReadSelection,
        cursor: Option<&ReadCursor>,
    ) -> Result<()> {
        if kind != selection.kind {
            return Err(Error::InvalidInput {
                field: "read selection",
                reason: "collection does not match the read method",
            });
        }
        self.check(selection, cursor)
    }

    pub fn check(&self, selection: &ReadSelection, cursor: Option<&ReadCursor>) -> Result<()> {
        if selection.object_versions == ObjectVersions::Latest && self.execution.is_none() {
            return Err(Error::InvalidInput {
                field: "object query",
                reason: "latest selection requires a bounded query view",
            });
        }
        if selection.source_time.is_some_and(|range| {
            range.start >= range.end
                || !matches!(selection.kind, ReadKind::Records | ReadKind::Stream)
        }) {
            return Err(Error::InvalidInput {
                field: "source time range",
                reason: "requires an increasing record or stream range",
            });
        }
        if (selection.object_versions == ObjectVersions::Latest
            && selection.kind != ReadKind::Objects)
            || (selection.object_key.is_some() && selection.kind != ReadKind::Objects)
            || (selection.item.is_some() && selection.kind == ReadKind::Objects)
            || (selection.history == ReadHistory::Retained
                && !matches!(selection.kind, ReadKind::Objects | ReadKind::Files))
            || (selection.stream.is_some() && selection.kind != ReadKind::Stream)
            || (selection.scope.is_some()
                && matches!(selection.kind, ReadKind::Schemas | ReadKind::Objects))
            || (selection.schema.is_some() && selection.kind == ReadKind::Files)
        {
            return Err(Error::InvalidInput {
                field: "read selection",
                reason: "filter does not apply to this collection",
            });
        }

        let through = selection.through.unwrap_or(self.saved.sequence);
        if through > self.saved.sequence || selection.after > through {
            return Err(Error::InvalidInput {
                field: "read selection",
                reason: "range lies outside the fixed view",
            });
        }

        if let Some(cursor) = cursor {
            self.accepted.history.check(&cursor.position.history)?;
            if cursor.view != self.id
                || cursor.selection != *selection
                || cursor.position.sequence <= selection.after
                || cursor.position.sequence > through
            {
                return Err(Error::InvalidInput {
                    field: "read cursor",
                    reason: "cursor does not belong to this view and selection",
                });
            }
        }

        Ok(())
    }
}

impl ReadCursor {
    pub fn new(view: &ReadView, selection: ReadSelection, position: StorageCursor) -> Result<Self> {
        let cursor = Self {
            view: view.id.clone(),
            selection,
            position,
        };
        view.check(&cursor.selection, Some(&cursor))?;
        Ok(cursor)
    }

    pub fn position(&self) -> &StorageCursor {
        &self.position
    }
}

impl<T> ReadPage<T> {
    pub fn check_positions<'a>(
        &'a self,
        view: &ReadView,
        selection: &ReadSelection,
        cursor: Option<&ReadCursor>,
        position: impl Fn(&'a T) -> &'a StorageCursor,
    ) -> Result<()> {
        view.check(selection, cursor)?;
        let through = selection.through.unwrap_or(view.saved.sequence);
        let mut previous = cursor.map(|cursor| cursor.position.sequence);

        for entry in &self.entries {
            let at = position(entry);
            view.accepted.history.check(&at.history)?;
            let ordered = previous.is_none_or(|previous| match selection.order {
                ReadOrder::Ascending => at.sequence > previous,
                ReadOrder::Descending => at.sequence < previous,
            });
            if at.sequence <= selection.after || at.sequence > through || !ordered {
                return Err(Error::InvalidInput {
                    field: "read page",
                    reason: "entry lies outside the ordered selection",
                });
            }
            previous = Some(at.sequence);
        }

        if let Some(next) = &self.next {
            view.check(selection, Some(next))?;
            if self.entries.last().map(position) != Some(next.position()) {
                return Err(Error::InvalidInput {
                    field: "read page",
                    reason: "continuation must follow the last returned entry",
                });
            }
        }

        Ok(())
    }

    pub fn check(
        &self,
        limits: ReadLimits,
        mut encoded_bytes: impl FnMut(&T) -> usize,
    ) -> Result<()> {
        let bytes = self.entries.iter().try_fold(0usize, |sum, entry| {
            sum.checked_add(encoded_bytes(entry))
                .ok_or(Error::InvalidInput {
                    field: "read page",
                    reason: "encoded size overflow",
                })
        })?;
        limits.check(self.entries.len(), bytes)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
