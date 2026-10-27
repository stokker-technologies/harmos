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

#[cfg(feature = "runtime")]
use crate::{Error, Operation, OperationContext, Response};
use harmos_domain::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message(harmos_path = crate)]
#[derive(Clone, Default, PartialEq)]
pub struct WatchRequest {
    pub instance: u64,
}

#[harmos::operation(harmos_path = crate, id = "host_operations_watch", request = WatchRequest,
    response = super::InstanceState, output = super::InstanceState, policy = super::WATCH_POLICY, infer_request,
    visibility = harmos_domain::operations::Visibility::Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Watch"))]
pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WatchRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Watch {
    type Request = WatchRequest;
    type Response = super::InstanceState;
    type Error = Error;
    type Endpoints = harmos_sdk::operations::StreamOutput<super::InstanceState>;
    async fn initialize(_: OperationContext) -> Result<Self, Error> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<Self::Response>, Error> {
        let access = context.dependency::<harmos_sdk::operations::instances::InstanceAccess>()?;
        let instance = access.0.owned(Id::new(request.instance)).ok_or(Error::UnknownInstance(request.instance))?;
        use harmos_domain::MemorySize;
        let mut changes = instance.subscribe();
        drop(instance);
        loop {
            let status = changes.current();
            let state = super::InstanceState::from(status);
            output.send_with(state.memory_size(), || Ok(state.clone())).await?;
            if status.is_settled() || !changes.changed().await {
                return Ok(state.into());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
