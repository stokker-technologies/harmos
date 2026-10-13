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
pub use harmos_contract::builtin::{Cancel, InstanceState, Status};

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::operations::{
    Context, Lane, Operation, OperationInstanceId, OperationManager, Response,
};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The built-in operations served in this slice: `host.operations.status` and
// `host.operations.cancel`.
pub struct Catalog;

// Reports an instance the caller submitted. Settled instances leave the index,
// so their status is unknown here; a live handle keeps reporting them.
pub struct Reporting;

// Requests cancellation of an instance the caller submitted.
pub struct Cancelling;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Catalog {
    pub fn register(
        operations: &OperationManager,
        owner: impl Into<String>,
        lane: impl Lane + Copy,
    ) -> Result<()> {
        let owner = owner.into();
        operations.register_wire_operation::<Reporting>(owner.clone(), lane, Arc::new(()))?;
        operations.register_wire_operation::<Cancelling>(owner, lane, Arc::new(()))
    }
}

impl Operation for Reporting {
    type Declaration = Status;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Status,
        _: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        let state = InstanceState::from(instance.status().portable());
        Ok(state.into())
    }
}

impl Operation for Cancelling {
    type Declaration = Cancel;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Cancel,
        _: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        instance.cancel();
        let state = InstanceState::from(instance.status().portable());
        Ok(state.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn unknown(id: OperationInstanceId) -> Error {
    Error::NotRegistered {
        domain: "operation instance",
        name: id.to_string(),
    }
}

/* === Tests === */
