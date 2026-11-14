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

use harmos_protocol::host::StatusGetRequest;
use harmos_protocol::host::StatusGetResponse;
use harmos_protocol::host::StatusWatchRequest;
use harmos_protocol::host::StatusWatchResponse;
use harmos_protocol::host::status::DiagnosticsGetRequest;
use harmos_protocol::host::status::DiagnosticsGetResponse;
use harmos_protocol::host::status::LogsListRequest;
use harmos_protocol::host::status::LogsListResponse;
use harmos_protocol::host::status::LogsWatchRequest;
use harmos_protocol::host::status::LogsWatchResponse;
use harmos_protocol::host::status::TraceEventsListRequest;
use harmos_protocol::host::status::TraceEventsListResponse;
use harmos_protocol::host::status::TraceEventsWatchRequest;
use harmos_protocol::host::status::TraceEventsWatchResponse;

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
    fn get(&self, request: StatusGetRequest) -> Pending<'_, StatusGetResponse>;
    fn watch(&self, request: StatusWatchRequest) -> Pending<'_, EventStream<StatusWatchResponse>>;
    fn logs_list(&self, request: LogsListRequest) -> Pending<'_, LogsListResponse>;
    fn logs_watch(&self, request: LogsWatchRequest) -> Pending<'_, EventStream<LogsWatchResponse>>;
    fn trace_events_list(&self, request: TraceEventsListRequest) -> Pending<'_, TraceEventsListResponse>;
    fn trace_events_watch(&self, request: TraceEventsWatchRequest) -> Pending<'_, EventStream<TraceEventsWatchResponse>>;
    fn diagnostics_get(&self, request: DiagnosticsGetRequest) -> Pending<'_, DiagnosticsGetResponse>;
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
