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
use harmos_domain::operations::instance::{Id, Status};
use harmos_operations::lifecycle::InstanceState;
use harmos_sdk::operations::{
    CallContext, ContextError, ContextValue, Host, Instance, OperationDeclaration, Response, WireChildAction, WireChildCall,
    WireChildControl,
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
        super::plugin::__bindings::harmos::guest::operation_context::child(payload).await.map_err(|error| {
            let code = error.code.parse().unwrap_or(UNKNOWN);
            match harmos_domain::Failure::decode(&error.failure) {
                Ok(failure) => OperationRefusal::failed(code, failure),
                Err(source) => OperationRefusal::new(code, source),
            }
        })
    }

    async fn child_call<O: OperationDeclaration>(&self, request: &O::Request, submit: bool) -> Result<Vec<u8>, OperationRefusal> {
        let context = self.calling_context()?;
        let target = self.assignments.resolve::<O>().map_err(OperationRefusal::of)?;
        let call = WireChildCall {
            context,
            generation_context: self.generation_context,
            values: self.call.portable(),
            operation: target.operation.clone(),
            fingerprint: target.fingerprint,
            generation: target.generation,
            payload: harmos_domain::Message::encode(request),
            submit,
        };
        let payload = call.encode().map_err(OperationRefusal::of)?;
        self.child(payload).await
    }

    fn control(&self, instance: Id, action: WireChildAction) -> Result<Vec<u8>, OperationRefusal> {
        let context = self.context.ok_or_else(|| OperationRefusal::new(UNKNOWN, "no guest attempt context"))?;
        let control = WireChildControl {
            context,
            instance: instance.value(),
            action,
        };
        control.encode().map_err(OperationRefusal::of)
    }

    async fn child_control(&self, instance: Id, action: WireChildAction) -> Result<Vec<u8>, OperationRefusal> {
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

impl harmos_sdk::operations::Caller for Context {
    fn call(
        &self, target: harmos_sdk::operations::Target, request: Box<dyn std::any::Any + Send>,
    ) -> harmos_sdk::operations::CallFuture<'_> {
        Box::pin(async move {
            let result = async move {
                let context = self.calling_context()?;
                let assigned = self.assignments.resolve_target(&target).map_err(OperationRefusal::of)?;
                let payload = (target.encode)(request.as_ref()).ok_or_else(|| OperationRefusal::new(UNKNOWN, "callee payload differs"))?;
                let command = WireChildCall {
                    context,
                    generation_context: self.generation_context,
                    values: self.call.portable(),
                    operation: assigned.operation.clone(),
                    fingerprint: assigned.fingerprint,
                    generation: assigned.generation,
                    payload,
                    submit: false,
                };
                let bytes = self.child(command.encode().map_err(OperationRefusal::of)?).await?;
                (target.decode)(&bytes).map_err(OperationRefusal::of)
            }
            .await;
            result.map_err(|error| harmos_sdk::operations::InvocationError::failed(target.id, error))
        })
    }
}

impl<O: OperationDeclaration> OperationInstance<O> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub async fn status(&self) -> Result<Status, OperationRefusal> {
        let bytes = self.host.child_control(self.id, WireChildAction::Status).await?;
        let state = InstanceState::decode(&bytes).map_err(OperationRefusal::of)?;
        Status::try_from(&state).map_err(OperationRefusal::of)
    }

    pub async fn request_cancel(&self) -> Result<(), OperationRefusal> {
        self.host.child_control(self.id, WireChildAction::Cancel).await?;
        Ok(())
    }

    pub async fn join(mut self) -> Result<Response<O::Response>, OperationRefusal> {
        let joined = self.host.child_control(self.id, WireChildAction::Join).await;
        self.released = true;
        let bytes = joined?;
        let response = O::Response::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(Response::new(response))
    }

    pub async fn cancel(self) -> Result<Response<O::Response>, OperationRefusal> {
        self.request_cancel().await?;
        self.join().await
    }

    // Keep the operation running without a handle until its submitting
    // attempt ends, which cancels and settles it like any other child.
    pub fn detach(mut self) -> Id {
        self.released = true;
        self.host.child_control_detached(self.id, WireChildAction::Detach);
        self.id
    }
}

impl<O> Drop for OperationInstance<O> {
    fn drop(&mut self) {
        if !self.released {
            self.host.child_control_detached(self.id, WireChildAction::Release);
        }
    }
}

impl<O: OperationDeclaration> Instance<O> for OperationInstance<O> {
    type Error = OperationRefusal;

    fn id(&self) -> Id {
        self.id
    }

    fn status(&self) -> impl Future<Output = Result<Status, OperationRefusal>> + Send {
        OperationInstance::status(self)
    }

    fn request_cancel(&self) -> impl Future<Output = Result<(), OperationRefusal>> + Send {
        OperationInstance::request_cancel(self)
    }

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, OperationRefusal>> + Send {
        OperationInstance::join(self)
    }

    fn detach(self) -> Id {
        OperationInstance::detach(self)
    }
}

impl Host for Context {
    fn publish_on<S: harmos_domain::StreamSpec>(&self, row: &S::Row) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        harmos_operations::streams::publish_on::<_, S>(self, row)
    }

    type Error = OperationRefusal;
    type Instance<O: OperationDeclaration> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Context::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        Context::with_value(self, value)
    }

    async fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<OperationInstance<O>, OperationRefusal> {
        let bytes = self.child_call::<O>(&request, true).await?;
        let id = u64::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    async fn submit_owned<O: OperationDeclaration>(
        &self, _: O::Request, _: harmos_sdk::operations::RequestClaims,
    ) -> Result<OperationInstance<O>, OperationRefusal> {
        Err(OperationRefusal::of(format!("`{}` needs process-local request ownership a guest cannot carry", O::ID)))
    }

    async fn call<O: OperationDeclaration>(&self, request: O::Request) -> Result<Response<O::Response>, OperationRefusal> {
        let bytes = self.child_call::<O>(&request, false).await?;
        let response = O::Response::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(Response::new(response))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
