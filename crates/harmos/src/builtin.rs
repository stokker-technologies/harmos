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
pub use harmos_domain::builtin::{Cancel, InstanceState, Status, Watch};

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::MemorySize;
use crate::operations::{
    Context, Lane, Operation, OperationInstanceId, OperationManager, Response, StreamOutput,
};
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
    pub fn register(
        operations: &OperationManager,
        owner: impl Into<String>,
        lane: impl Lane + Copy,
    ) -> Result<()> {
        let owner = owner.into();
        operations.register_wire_operation::<Reporting>(owner.clone(), lane, Arc::new(()))?;
        operations.register_wire_operation::<Cancelling>(owner.clone(), lane, Arc::new(()))?;
        operations.register_wire_operation::<Watching>(owner, lane, Arc::new(()))
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

impl Operation for Watching {
    type Declaration = Watch;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<InstanceState>;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Watch,
        output: &mut Self::Endpoints,
    ) -> Result<Response<InstanceState>> {
        let id = OperationInstanceId::new(request.instance);
        let instance = context.owned_instance(id).ok_or_else(|| unknown(id))?;
        // Subscribe before reading so no change between them is lost.
        let mut changes = instance.settlement();
        drop(instance);
        loop {
            let status = changes.borrow_and_update().portable();
            let state = InstanceState::from(status);
            output
                .send_with(state.memory_size(), || Ok(state.clone()))
                .await?;
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
    Error::NotRegistered {
        domain: "operation instance",
        name: id.to_string(),
    }
}

/* === Tests === */
