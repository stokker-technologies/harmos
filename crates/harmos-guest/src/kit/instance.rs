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
use crate::envelope::{WireChildAction, WireChildCall, WireChildControl};
use crate::lifecycle::InstanceState;
use harmos_domain::Message;
use harmos_operations::execution::operations::{CallContext, ContextError, ContextValue, Instance, OperationHost, RequestShape, Response};
use harmos_operations::operations::instance::{Id, Status};

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

pub(super) struct Child {
    pub(super) host: Context,
    pub(super) id: Id,
    pub(super) target: harmos_operations::execution::operations::Target,
    pub(super) released: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Context {
    async fn child(&self, payload: Vec<u8>) -> Result<Vec<u8>, OperationRefusal> {
        super::plugin::__bindings::harmos::guest::operation_context::child(payload).await.map_err(|error| {
            let code = error.code.parse().unwrap_or(UNKNOWN);
            match harmos_operations::Failure::decode(&error.failure) {
                Ok(failure) => OperationRefusal::failed(code, failure),
                Err(source) => OperationRefusal::new(code, source),
            }
        })
    }

    async fn child_call<O: RequestShape>(
        &self, request: &O::Request, submit: bool, retry: Option<harmos_operations::execution::operations::RetryPolicy>,
    ) -> Result<Vec<u8>, OperationRefusal> {
        let context = self.calling_context()?;
        let target = self.assignments.resolve::<O>().map_err(OperationRefusal::of)?;
        let call = WireChildCall {
            context,
            generation_context: self.generation_context,
            values: self.call.portable(),
            operation: target.operation.clone(),
            fingerprint: target.fingerprint.as_slice().try_into().map_err(OperationRefusal::of)?,
            generation: target.generation,
            payload: harmos_domain::Message::encode(request),
            submit,
            retry,
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

impl harmos_operations::execution::operations::Caller for Context {
    fn start(
        &self, target: harmos_operations::execution::operations::Target, request: Box<dyn std::any::Any + Send>,
        retry: Option<harmos_operations::execution::operations::RetryPolicy>, endpoints: Box<dyn std::any::Any + Send>, call: CallContext,
    ) -> harmos_operations::execution::operations::StartFuture<'_> {
        Box::pin(async move {
            let id = target.id;
            let result = async move {
                if !endpoints.is::<()>() {
                    return Err(OperationRefusal::new(UNKNOWN, "native endpoints are unavailable across the guest boundary"));
                }
                let context = self.calling_context()?;
                let assigned = self.assignments.resolve_target(&target).map_err(OperationRefusal::of)?;
                let payload = (target.encode)(request.as_ref()).ok_or_else(|| OperationRefusal::new(UNKNOWN, "callee payload differs"))?;
                let command = WireChildCall {
                    context,
                    generation_context: self.generation_context,
                    values: call.portable(),
                    operation: assigned.operation.clone(),
                    fingerprint: assigned.fingerprint.as_slice().try_into().map_err(OperationRefusal::of)?,
                    generation: assigned.generation,
                    payload,
                    submit: true,
                    retry,
                };
                let bytes = self.child(command.encode().map_err(OperationRefusal::of)?).await?;
                let id = u64::decode(&bytes).map_err(OperationRefusal::of)?;
                Ok(Box::new(Child {
                    host: self.clone(),
                    id: Id::new(id),
                    target,
                    released: false,
                }) as Box<dyn harmos_operations::execution::operations::ChildInstance>)
            }
            .await;
            result.map_err(|error| harmos_operations::execution::operations::InvocationError::failed(id, error))
        })
    }
}

impl<O: RequestShape> OperationInstance<O> {
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

impl<O: RequestShape> Instance<O> for OperationInstance<O> {
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

impl OperationHost for Context {
    async fn resolve<O: RequestShape, E: harmos_operations::execution::operations::EndpointSet>(
        &self, request: O::Request, retry: Option<harmos_operations::execution::operations::RetryPolicy>, _endpoints: E,
    ) -> Result<Response<O::Response>, OperationRefusal> {
        if E::item_types() != (None, None) {
            return Err(OperationRefusal::of("guest calls do not accept native endpoints"));
        }
        let bytes = self.child_call::<O>(&request, false, retry).await?;
        O::Response::decode(&bytes).map(Response::new).map_err(OperationRefusal::of)
    }

    async fn invoke<O: RequestShape, E: harmos_operations::execution::operations::EndpointSet>(
        &self, request: O::Request, retry: Option<harmos_operations::execution::operations::RetryPolicy>, _endpoints: E,
    ) -> Result<Self::Instance<O>, OperationRefusal> {
        if E::item_types() != (None, None) {
            return Err(OperationRefusal::new(UNKNOWN, "native endpoints are unavailable across the guest boundary"));
        }
        let bytes = self.child_call::<O>(&request, true, retry).await?;
        let id = u64::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    fn publish_on<S: harmos_operations::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        crate::builtins::streams::publish_on::<_, S>(self, row)
    }

    type Error = OperationRefusal;
    type Instance<O: RequestShape> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Context::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        Context::with_value(self, value)
    }

    async fn submit<O: RequestShape>(&self, request: O::Request) -> Result<OperationInstance<O>, OperationRefusal> {
        let bytes = self.child_call::<O>(&request, true, None).await?;
        let id = u64::decode(&bytes).map_err(OperationRefusal::of)?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    async fn submit_owned<O: RequestShape>(
        &self, _: O::Request, _: harmos_operations::execution::operations::RequestClaims,
    ) -> Result<OperationInstance<O>, OperationRefusal> {
        Err(OperationRefusal::of("a guest cannot carry process-local request ownership"))
    }
}

impl harmos_operations::execution::operations::ChildInstance for Child {
    fn id(&self) -> Id {
        self.id
    }
    fn status(&self) -> harmos_operations::execution::operations::ControlFuture<'_, Status> {
        Box::pin(async move {
            let result = async {
                let bytes = self.host.child_control(self.id, WireChildAction::Status).await?;
                InstanceState::decode(&bytes)
                    .map_err(OperationRefusal::of)
                    .and_then(|state| Status::try_from(&state).map_err(OperationRefusal::of))
            }
            .await;
            result
                .map_err(|error: OperationRefusal| harmos_operations::execution::operations::InvocationError::failed(self.target.id, error))
        })
    }
    fn request_cancel(&self) -> harmos_operations::execution::operations::ControlFuture<'_, ()> {
        Box::pin(async move {
            self.host
                .child_control(self.id, WireChildAction::Cancel)
                .await
                .map(|_| ())
                .map_err(|error| harmos_operations::execution::operations::InvocationError::failed(self.target.id, error))
        })
    }
    fn join(mut self: Box<Self>) -> harmos_operations::execution::operations::CallFuture<'static> {
        Box::pin(async move {
            let bytes = self
                .host
                .child_control(self.id, WireChildAction::Join)
                .await
                .map_err(|error| harmos_operations::execution::operations::InvocationError::failed(self.target.id, error))?;
            self.released = true;
            (self.target.decode)(&bytes)
                .map_err(|error| harmos_operations::execution::operations::InvocationError::failed(self.target.id, error))
        })
    }
    fn detach(mut self: Box<Self>) -> Id {
        self.host.child_control_detached(self.id, WireChildAction::Detach);
        self.released = true;
        self.id
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if !self.released {
            self.host.child_control_detached(self.id, WireChildAction::Release);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
