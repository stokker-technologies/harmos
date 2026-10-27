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
pub struct CancelRequest {
    pub instance: u64,
}

#[harmos::operation(harmos_path = crate, id = "host_operations_cancel", request = CancelRequest,
    response = super::InstanceState, policy = super::POLICY, infer_request,
    visibility = harmos_domain::operations::Visibility::Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Cancel"))]
pub struct Cancel;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl CancelRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Cancel {
    type Request = CancelRequest;
    type Response = super::InstanceState;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(_: OperationContext) -> Result<Self, Error> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<Self::Response>, Error> {
        let access = context.dependency::<harmos_sdk::operations::instances::InstanceAccess>()?;
        let instance = access.0.owned(Id::new(request.instance)).ok_or(Error::UnknownInstance(request.instance))?;
        instance.cancel();
        let _ = output;
        Ok(super::InstanceState::from(instance.status()).into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
