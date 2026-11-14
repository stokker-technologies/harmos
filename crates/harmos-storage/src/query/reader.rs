/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Admitted Query Readers and Schema Resolution

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use crate::backend::ScanPage;
use crate::execution::Context;
use crate::manager::Inner;
use crate::{Error, Result, SchemaKey, Snapshot, StorageSchema};
use harmos_domain::model::storage::query::QueryCompletion;
use harmos_query::{QueryArrow, QueryReader as EngineReader, QueryStep};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct QueryReader {
    inner: Arc<Inner>,
    reader: EngineReader,
    active: Option<OwnedSemaphorePermit>,
    context: Context,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl QueryReader {
    pub(super) fn new(inner: Arc<Inner>, reader: EngineReader, active: OwnedSemaphorePermit) -> Self {
        Self {
            inner,
            reader,
            active: Some(active),
            context: Context::current(),
        }
    }

    pub fn configure_page(&mut self, rows: NonZeroUsize, bytes: NonZeroUsize) {
        self.context = Context::current();
        self.reader.configure_page(rows, bytes);
    }

    pub fn suspend_page(&mut self) {
        self.active = None;
    }

    pub async fn next(&mut self) -> Result<Option<QueryArrow>> {
        loop {
            match self.step().await? {
                QueryStep::Output(output) => return Ok(Some(output)),
                QueryStep::Progress => {}
                QueryStep::Complete => return Ok(None),
            }
        }
    }

    pub async fn step(&mut self) -> Result<QueryStep> {
        if self.active.is_none() {
            self.active = Some(query_slot(&self.inner).await?);
        }
        let result = self.context.scope(self.reader.step()).await;
        if matches!(&result, Ok(QueryStep::Complete) | Err(_)) {
            self.active = None;
        }
        Ok(result?)
    }

    pub fn completion(&self) -> Result<QueryCompletion> {
        Ok(self.reader.completion()?)
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(crate) async fn schema_at(inner: &Arc<Inner>, snapshot: &Snapshot, key: &SchemaKey) -> Result<StorageSchema> {
    find_schema(inner, snapshot, key).await?.ok_or(Error::InvalidInput {
        field: "storage schema",
        reason: "schema is unavailable at requested cursor",
    })
}
pub(crate) async fn find_schema(inner: &Arc<Inner>, snapshot: &Snapshot, key: &SchemaKey) -> Result<Option<StorageSchema>> {
    let selection = super::SchemaSelection { schema: Some(key.clone()) };
    let page = ScanPage {
        after: 0,
        rows: NonZeroUsize::MIN,
        bytes: inner.settings.scan_bytes(),
    };
    let (mut schemas, _) = snapshot.scan::<StorageSchema>(&selection, page).await?;
    Ok(schemas.pop())
}
pub(crate) async fn query_slot(inner: &Arc<Inner>) -> Result<OwnedSemaphorePermit> {
    if let Ok(active) = Arc::clone(&inner.query_active).try_acquire_owned() {
        return Ok(active);
    }
    let _pending = Arc::clone(&inner.query_pending)
        .try_acquire_owned()
        .map_err(|_| Error::ResourceExhausted { resource: "pending storage queries" })?;
    Arc::clone(&inner.query_active).acquire_owned().await.map_err(|_| Error::Closing { domain: "storage queries" })
}
/* === Tests === */
