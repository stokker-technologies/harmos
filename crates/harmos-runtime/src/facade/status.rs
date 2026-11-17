/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Status Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::host::status::{
    DiagnosticsGetRequest, DiagnosticsGetResponse, LogsListRequest, LogsListResponse, LogsWatchRequest, LogsWatchResponse,
    TraceEventsListRequest, TraceEventsListResponse, TraceEventsWatchRequest, TraceEventsWatchResponse,
};
pub use harmos_protocol::host::{StatusGetRequest, StatusGetResponse, StatusWatchRequest, StatusWatchResponse};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use crate::operations::{NativeInvocation, NativeOperation};
use harmos_operations::host::declarations::status;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Status<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Status<'_> {
    pub async fn get(&self, request: StatusGetRequest) -> Result<StatusGetResponse> {
        self.call.call::<status::Get>(request).await
    }

    pub fn watch(
        &self, request: StatusWatchRequest, output: <status::Watch as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<status::Watch>> {
        self.call.submit::<status::Watch>(request, output)
    }

    pub async fn logs_list(&self, request: LogsListRequest) -> Result<LogsListResponse> {
        self.call.call::<status::LogsList>(request).await
    }

    pub fn logs_watch(
        &self, request: LogsWatchRequest, output: <status::LogsWatch as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<status::LogsWatch>> {
        self.call.submit::<status::LogsWatch>(request, output)
    }

    pub async fn trace_events_list(&self, request: TraceEventsListRequest) -> Result<TraceEventsListResponse> {
        self.call.call::<status::TraceEventsList>(request).await
    }

    pub fn trace_events_watch(
        &self, request: TraceEventsWatchRequest, output: <status::TraceEventsWatch as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<status::TraceEventsWatch>> {
        self.call.submit::<status::TraceEventsWatch>(request, output)
    }

    pub async fn diagnostics_get(&self, request: DiagnosticsGetRequest) -> Result<DiagnosticsGetResponse> {
        self.call.call::<status::DiagnosticsGet>(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
