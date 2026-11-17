/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Invocation Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::host::{
    InvocationsCancelRequest, InvocationsCancelResponse, InvocationsCreateRequest, InvocationsCreateResponse, InvocationsGetRequest,
    InvocationsGetResponse, InvocationsWatchRequest, InvocationsWatchResponse,
};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use crate::operations::{NativeInvocation, NativeOperation};
use harmos_operations::host::declarations::lifecycle;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Invocations<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Invocations<'_> {
    pub async fn create(&self, request: InvocationsCreateRequest) -> Result<InvocationsCreateResponse> {
        self.call.call::<lifecycle::Create>(request).await
    }

    pub async fn get(&self, request: InvocationsGetRequest) -> Result<InvocationsGetResponse> {
        self.call.call::<lifecycle::Get>(request).await
    }

    pub async fn cancel(&self, request: InvocationsCancelRequest) -> Result<InvocationsCancelResponse> {
        self.call.call::<lifecycle::Cancel>(request).await
    }

    pub fn watch(
        &self, request: InvocationsWatchRequest, output: <lifecycle::Watch as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<lifecycle::Watch>> {
        self.call.submit::<lifecycle::Watch>(request, output)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
