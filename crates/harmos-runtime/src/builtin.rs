/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Operations Harmos implements over its own state. Nothing here is served by
// default: a host registers the catalog explicitly, after which plugins reach
// it by id through the ordinary call path and gRPC through its declared routes.

/* === Modules === */

/* === Re-Exports === */

pub use harmos_domain::operations::lifecycle::{Cancel, CancelRequest, InstanceState, Status, StatusRequest, Watch, WatchRequest};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::MemorySize;
use crate::operations::{Operation, OperationInstanceId, Response, StreamOutput};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The built-in operations served so far: `host.operations.status`,
// `host.operations.cancel` and `host.operations.watch`.
pub struct Catalog;

// Reports an instance the caller submitted. Settled instances leave the index,
// so their status is unknown here; a live handle keeps reporting them.
pub struct Reporting;

// Requests cancellation of an instance the caller submitted.
pub struct Cancelling;

// Streams each status change of an instance the caller submitted, from its
// status when the watch begins until it stops or settles.
pub struct Watching;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Catalog {
    pub fn registration() -> harmos_sdk::RegistrationBatch {
        harmos_sdk::RegistrationBatch::new(harmos_sdk::Dependencies::new())
            .implementation::<Status, Reporting>()
            .implementation::<Cancel, Cancelling>()
            .implementation::<Watch, Watching>()
    }
}
impl Operation for Reporting {
    type Request = <Status as harmos_domain::OperationDeclaration>::Request;
    type Response = <Status as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(_: harmos_sdk::OperationContext) -> Result<Self> {
        Ok(Self)
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let host = context.dependency::<crate::operations::HostContext>()?;
        let context = host.borrow();
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        let state = InstanceState::from(instance.status().portable());
        Ok(state.into())
    }
}

impl Operation for Cancelling {
    type Request = <Cancel as harmos_domain::OperationDeclaration>::Request;
    type Response = <Cancel as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(_: harmos_sdk::OperationContext) -> Result<Self> {
        Ok(Self)
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let host = context.dependency::<crate::operations::HostContext>()?;
        let context = host.borrow();
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        instance.cancel();
        let state = InstanceState::from(instance.status().portable());
        Ok(state.into())
    }
}

impl Operation for Watching {
    type Request = <Watch as harmos_domain::OperationDeclaration>::Request;
    type Response = <Watch as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<InstanceState>;

    async fn initialize(_: harmos_sdk::OperationContext) -> Result<Self> {
        Ok(Self)
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let host = context.dependency::<crate::operations::HostContext>()?;
        let context = host.borrow();
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        // Subscribe before reading so no change between them is lost.
        let mut changes = instance.settlement();
        drop(instance);
        loop {
            let status = changes.borrow_and_update().portable();
            let state = InstanceState::from(status);
            output.send_with(state.memory_size(), || Ok(state.clone())).await?;
            if status.is_settled() {
                return Ok(state.into());
            }
            if changes.changed().await.is_err() {
                return Ok(state.into());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn unknown(id: OperationInstanceId) -> Error {
    Error::NotRegistered { domain: "operation instance", name: id.to_string() }
}

/* === Tests === */
