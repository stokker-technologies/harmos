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
use std::marker::PhantomData;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{
    CallError, ObserverContext, ObserverHost, ObserverPublisher, ObserverStreamPublisher,
    PublicationError, Supervisor,
};
use harmos_contract::{Message, StreamSpec, plugin::Declared};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Publisher<P> {
    supervisor: Supervisor,
    _plugin: PhantomData<fn() -> P>,
}

// Rows for one stream, each stamped with the time it was produced: `push`
// reads the kit clock, never earlier than the previous stamp, and `push_at`
// takes an explicit stamp. Publishing sends the stamps, not the send time.
pub struct Batch<S: StreamSpec> {
    rows: Vec<S::Row>,
    stamps: Vec<Option<i64>>,
    latest: Option<i64>,
    clock: fn() -> Option<i64>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    Publication(#[from] PublicationError),
    #[error(transparent)]
    Call(#[from] CallError),
}

/* === Traits === */

/* === Implementations === */
impl From<PublishError> for harmos_contract::Refusal {
    fn from(error: PublishError) -> Self {
        match error {
            PublishError::Call(error) => error.into(),
            error => Self::of(error),
        }
    }
}

impl<P> Clone for Publisher<P> {
    fn clone(&self) -> Self {
        Self {
            supervisor: self.supervisor.clone(),
            _plugin: PhantomData,
        }
    }
}

impl Supervisor {
    pub fn publisher<P: Declared>(&self) -> Publisher<P> {
        Publisher {
            supervisor: self.clone(),
            _plugin: PhantomData,
        }
    }
}

impl<P: Declared> Publisher<P> {
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    pub async fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        self.publish_on_context::<S>(row, None).await
    }

    pub async fn publish_on_context<S: StreamSpec>(
        &self,
        row: &S::Row,
        context: Option<&str>,
    ) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        Self::declared::<S>()?;
        let stamps = [self.supervisor.now()];
        self.supervisor
            .publication::<S>(std::slice::from_ref(row), &stamps, context)
            .await
    }

    pub fn batch<S: StreamSpec>(&self) -> Batch<S> {
        Batch {
            rows: Vec::new(),
            stamps: Vec::new(),
            latest: None,
            clock: self.supervisor.clock(),
        }
    }

    pub async fn publish_batch_on<S: StreamSpec>(
        &self,
        batch: &Batch<S>,
    ) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        self.publish_batch_on_context::<S>(batch, None).await
    }

    // Publish rows in order with contiguous sequences and their own stamps,
    // as one `host.publish` call where they fit (at most one window of
    // rows). Each call answers once the host's ingest has taken in every row
    // published before it on the stream, so no row is dropped unread. Rows
    // before a refused row stay published.
    pub async fn publish_batch_on_context<S: StreamSpec>(
        &self,
        batch: &Batch<S>,
        context: Option<&str>,
    ) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        Self::declared::<S>()?;
        self.supervisor
            .publication::<S>(&batch.rows, &batch.stamps, context)
            .await
    }

    fn declared<S: StreamSpec>() -> Result<(), PublicationError> {
        if P::STREAMS
            .iter()
            .any(|entry| entry.id == S::ID && entry.schema == S::VERSION)
        {
            Ok(())
        } else {
            Err(PublicationError::Undeclared { stream: S::ID })
        }
    }
}

impl<S: StreamSpec> Batch<S> {
    pub fn push(&mut self, row: S::Row) {
        let stamp = match ((self.clock)(), self.latest) {
            (Some(now), Some(latest)) => Some(now.max(latest)),
            (now, latest) => now.or(latest),
        };
        self.stamped(row, stamp);
    }

    pub fn push_at(&mut self, row: S::Row, published_unix_micros: i64) {
        self.stamped(row, Some(published_unix_micros));
    }

    pub fn rows(&self) -> &[S::Row] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    // Empty the batch for reuse; later `push` stamps still never go back.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.stamps.clear();
    }

    fn stamped(&mut self, row: S::Row, stamp: Option<i64>) {
        self.latest = self.latest.max(stamp);
        self.rows.push(row);
        self.stamps.push(stamp);
    }
}

impl<R, P: Declared> ObserverHost<R> for Publisher<P> {
    fn bind(_: Arc<R>, supervisor: Supervisor) -> Self {
        supervisor.publisher::<P>()
    }
}

impl<R: Sync, V: StreamSpec<Row = V> + harmos_contract::schema::Described + Message, P: Declared>
    ObserverPublisher<R, V> for Publisher<P>
{
    type Error = PublishError;
    const STREAM: Option<harmos_contract::inventory::publications::Entry> =
        Some(harmos_contract::inventory::publications::Entry::of::<V>());

    async fn publish(
        &self,
        _: &ObserverContext<'_, R, Self>,
        value: V,
    ) -> Result<(), PublishError> {
        self.publish_on::<V>(&value).await
    }
}

impl<R: Sync, S: StreamSpec, P: Declared> ObserverStreamPublisher<R, S> for Publisher<P>
where
    S::Row: Message,
{
    type Error = PublishError;

    async fn publish_on(
        &self,
        _: &ObserverContext<'_, R, Self>,
        row: &S::Row,
    ) -> Result<(), PublishError> {
        Publisher::publish_on::<S>(self, row).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
