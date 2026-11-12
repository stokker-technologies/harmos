/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod diagnostics_get;
mod get;
mod logs_list;
mod logs_watch;
mod trace_events_list;
mod trace_events_watch;
mod watch;

/* === Re-Exports === */

pub use diagnostics_get::DiagnosticsGet;
pub use get::Get;
pub use logs_list::LogsList;
pub use logs_watch::LogsWatch;
pub use trace_events_list::TraceEventsList;
pub use trace_events_watch::TraceEventsWatch;
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

// Third Party Imports

// Local Imports
use crate::execution::operations::ServingError;

/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    cancellation_timeout: Duration::ZERO,
    retries: crate::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

pub type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;
pub type EventStream<T> = Pin<Box<dyn futures_core::Stream<Item = Result<T, ServingError>> + Send>>;

/* === Structs === */

#[derive(Default)]
pub struct StatusAccess(pub std::sync::OnceLock<Arc<dyn Status>>);

/* === Enums === */

/* === Traits === */

pub trait Status: Send + Sync {
    fn get(&self, request: harmos_protocol::v1::StatusGetRequest) -> Pending<'_, harmos_protocol::v1::StatusGetResponse>;
    fn watch(&self, request: harmos_protocol::v1::StatusWatchRequest)
    -> Pending<'_, EventStream<harmos_protocol::v1::StatusWatchResponse>>;
    fn logs_list(&self, request: harmos_protocol::status::LogsListRequest) -> Pending<'_, harmos_protocol::status::LogsListResponse>;
    fn logs_watch(
        &self, request: harmos_protocol::status::LogsWatchRequest,
    ) -> Pending<'_, EventStream<harmos_protocol::status::LogsWatchResponse>>;
    fn trace_events_list(
        &self, request: harmos_protocol::status::TraceEventsListRequest,
    ) -> Pending<'_, harmos_protocol::status::TraceEventsListResponse>;
    fn trace_events_watch(
        &self, request: harmos_protocol::status::TraceEventsWatchRequest,
    ) -> Pending<'_, EventStream<harmos_protocol::status::TraceEventsWatchResponse>>;
    fn diagnostics_get(
        &self, request: harmos_protocol::status::DiagnosticsGetRequest,
    ) -> Pending<'_, harmos_protocol::status::DiagnosticsGetResponse>;
}

/* === Implementations === */

/* === Public Functions === */

pub fn catalog() -> Result<crate::execution::__private::Catalog, ServingError> {
    use crate::execution::operations::OperationFactory;
    let catalog = crate::execution::__private::Catalog::new();
    let catalog = catalog.entry(OperationFactory::protobuf::<Get>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<Watch>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<LogsList>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<LogsWatch>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<TraceEventsList>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<TraceEventsWatch>()?);
    let catalog = catalog.entry(OperationFactory::protobuf::<DiagnosticsGet>()?);
    Ok(catalog)
}

/* === Private Functions === */

/* === Tests === */
