/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest binding of the portable host over the `child` Wasm import.
// Unary operations support submit with status, cancel and join; streaming
// sessions are not available to guests yet.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use super::UNKNOWN;
use super::context::Context;
use crate::OperationRefusal;
use harmos_domain::Message;
use harmos_domain::builtin::InstanceState;
use harmos_domain::operations::instance::{Id, Status};
use harmos_domain::operations::{
    CallContext, Callable, ContextError, ContextValue, Host, Instance, Response, WireChildAction,
    WireChildCall, WireChildControl,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[must_use = "dropping an unsettled operation instance requests its cancellation"]
pub struct OperationInstance<O> {
    host: Context,
    id: Id,
    released: bool,
    _operation: PhantomData<fn() -> O>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Context {
    async fn child(&self, payload: Vec<u8>) -> Result<Vec<u8>, OperationRefusal> {
        super::plugin::__bindings::harmos::guest::operation_context::child(payload)
            .await
            .map_err(|error| {
                let code = error.code.parse().unwrap_or(UNKNOWN);
                match harmos_domain::Failure::decode(&error.failure) {
                    Ok(failure) => OperationRefusal::failed(code, failure),
                    Err(source) => OperationRefusal::new(code, source),
                }
            })
    }

    async fn child_call<O: Callable>(
        &self,
        request: &O::Request,
        submit: bool,
    ) -> Result<Vec<u8>, OperationRefusal> {
        let context = self.calling_context()?;
        let call = WireChildCall {
            context,
            generation_context: self.generation_context,
            values: self.call.portable(),
            operation: O::ID.to_owned(),
            payload: request.encode_to_vec(),
            submit,
        };
        let payload = call.encode().map_err(OperationRefusal::of)?;
        self.child(payload).await
    }

    fn control(&self, instance: Id, action: WireChildAction) -> Result<Vec<u8>, OperationRefusal> {
        let context = self
            .context
            .ok_or_else(|| OperationRefusal::new(UNKNOWN, "no guest attempt context"))?;
        let control = WireChildControl {
            context,
            instance: instance.value(),
            action,
        };
        control.encode().map_err(OperationRefusal::of)
    }

    async fn child_control(
        &self,
        instance: Id,
        action: WireChildAction,
    ) -> Result<Vec<u8>, OperationRefusal> {
        let payload = self.control(instance, action)?;
        self.child(payload).await
    }

    // The handle is gone, so the control continues after the current poll.
    fn child_control_detached(&self, instance: Id, action: WireChildAction) {
        let Ok(payload) = self.control(instance, action) else {
            return;
        };
        let host = self.clone();
        wit_bindgen::spawn_local(async move {
            let _ = host.child(payload).await;
        });
    }
}

impl<O: Callable> OperationInstance<O> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub async fn status(&self) -> Result<Status, OperationRefusal> {
        let bytes = self
            .host
            .child_control(self.id, WireChildAction::Status)
            .await?;
        let state = InstanceState::decode(&bytes).map_err(OperationRefusal::of)?;
        Status::try_from(&state).map_err(OperationRefusal::of)
    }

    pub async fn cancel(&self) -> Result<(), OperationRefusal> {
        self.host
            .child_control(self.id, WireChildAction::Cancel)
            .await?;
        Ok(())
    }

    pub async fn join(mut self) -> Result<Response<O::Response>, OperationRefusal> {
        let joined = self
            .host
            .child_control(self.id, WireChildAction::Join)
            .await;
        self.released = true;
        let bytes = joined?;
        let response = O::Response::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(Response::new(response))
    }

    pub async fn cancel_and_join(self) -> Result<Response<O::Response>, OperationRefusal> {
        self.cancel().await?;
        self.join().await
    }

    // Keep the operation running without a handle until its submitting
    // attempt ends, which cancels and settles it like any other child.
    pub fn detach(mut self) -> Id {
        self.released = true;
        self.host
            .child_control_detached(self.id, WireChildAction::Detach);
        self.id
    }
}

impl<O> Drop for OperationInstance<O> {
    fn drop(&mut self) {
        if !self.released {
            self.host
                .child_control_detached(self.id, WireChildAction::Release);
        }
    }
}

impl<O: Callable> Instance<O> for OperationInstance<O> {
    type Error = OperationRefusal;

    fn id(&self) -> Id {
        self.id
    }

    fn status(&self) -> impl Future<Output = Result<Status, OperationRefusal>> + Send {
        OperationInstance::status(self)
    }

    fn cancel(&self) -> impl Future<Output = Result<(), OperationRefusal>> + Send {
        OperationInstance::cancel(self)
    }

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, OperationRefusal>> + Send {
        OperationInstance::join(self)
    }

    fn detach(self) -> Id {
        OperationInstance::detach(self)
    }
}

impl Host for Context {
    type Error = OperationRefusal;
    type Instance<O: Callable> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Context::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        Context::with_value(self, value)
    }

    async fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> Result<OperationInstance<O>, OperationRefusal> {
        let bytes = self.child_call::<O>(&request, true).await?;
        let id = u64::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    // Resident resources live in the host process; a guest cannot lend them.
    async fn submit_with_resources<O: Callable<Response = ()>, R: Send + Sync + 'static>(
        &self,
        _: O::Request,
        _: std::sync::Arc<R>,
    ) -> Result<OperationInstance<O>, OperationRefusal> {
        Err(OperationRefusal::of(format!(
            "`{}` needs process-local resources a guest cannot carry",
            O::ID
        )))
    }

    async fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> Result<Response<O::Response>, OperationRefusal> {
        let bytes = self.child_call::<O>(&request, false).await?;
        let response = O::Response::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(Response::new(response))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
