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
use std::future::Future;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{ObserverContext, PublicationError, Supervisor};
use harmos_domain::{StreamSpec, inventory::publications};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait ObserverHost: Send + Sync + Sized + 'static {
    fn bind(resources: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self;
}

pub trait ObserverPublisher<V: Send>: Send + Sync + Sized {
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;
    const STREAM: Option<publications::Entry> = None;

    fn publish(&self, context: &ObserverContext<'_, Self>, value: V) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait ObserverStreamPublisher<S: StreamSpec>: Send + Sync + Sized
where
    S::Row: Sync,
{
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;

    fn publish_on(&self, context: &ObserverContext<'_, Self>, row: &S::Row) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/* === Implementations === */

impl ObserverHost for Supervisor {
    fn bind(_: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self {
        supervisor
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
