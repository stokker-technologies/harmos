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
        if !P::STREAMS
            .iter()
            .any(|entry| entry.id == S::ID && entry.schema == S::VERSION)
        {
            return Err(PublicationError::Undeclared { stream: S::ID }.into());
        }
        self.supervisor.publication::<S>(row, context).await
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
