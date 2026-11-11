/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Sidecar binding of the portable host over the connection frames.
// A submitted child stays owned by the calling attempt; its handle drives it
// through child controls and releases it when dropped unsettled.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use super::{CallError, Supervisor};
use harmos_domain::operations::instance::{Id, Status};
use harmos_domain::{Message, StreamSpec};
use harmos_operations::execution::operations::{
    CallContext, ContextError, ContextValue, Instance, OperationDeclaration, OperationHost, Response, WireChildAction,
};
use harmos_operations::lifecycle::InstanceState;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "dropping an unsettled operation instance requests its cancellation"]
pub struct OperationInstance<O> {
    host: Supervisor,
    id: Id,
    released: bool,
    _operation: PhantomData<fn() -> O>,
}

pub(super) struct Child {
    pub(super) host: Supervisor,
    pub(super) id: Id,
    pub(super) target: harmos_operations::execution::operations::Target,
    pub(super) released: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<O: OperationDeclaration> OperationInstance<O> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub async fn status(&self) -> Result<Status, CallError> {
        let bytes = self.host.child_control(self.id, WireChildAction::Status).await?;
        decode_status(&bytes)
    }

    pub async fn request_cancel(&self) -> Result<(), CallError> {
        self.host.child_control(self.id, WireChildAction::Cancel).await?;
        Ok(())
    }

    pub async fn join(mut self) -> Result<Response<O::Response>, CallError> {
        let joined = self.host.child_control(self.id, WireChildAction::Join).await;
        self.released = true;
        let bytes = joined?;
        decode_response::<O>(&bytes)
    }

    pub async fn cancel(self) -> Result<Response<O::Response>, CallError> {
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
    type Error = CallError;

    fn id(&self) -> Id {
        self.id
    }

    fn status(&self) -> impl Future<Output = Result<Status, CallError>> + Send {
        OperationInstance::status(self)
    }

    fn request_cancel(&self) -> impl Future<Output = Result<(), CallError>> + Send {
        OperationInstance::request_cancel(self)
    }

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, CallError>> + Send {
        OperationInstance::join(self)
    }

    fn detach(self) -> Id {
        OperationInstance::detach(self)
    }
}

impl OperationHost for Supervisor {
    async fn resolve<O: OperationDeclaration, E: harmos_operations::execution::operations::EndpointSet>(
        &self, request: O::Request, retry: Option<harmos_operations::execution::operations::RetryPolicy>, _endpoints: E,
    ) -> Result<Response<O::Response>, CallError> {
        if E::item_types() != (None, None) {
            return Err(CallError::Unavailable);
        }
        let bytes = self.child_call::<O>(&request, false, retry).await?;
        decode_response::<O>(&bytes)
    }

    async fn invoke<O: OperationDeclaration, E: harmos_operations::execution::operations::EndpointSet>(
        &self, request: O::Request, retry: Option<harmos_operations::execution::operations::RetryPolicy>, _endpoints: E,
    ) -> Result<Self::Instance<O>, CallError> {
        if E::item_types() != (None, None) {
            return Err(CallError::Unavailable);
        }
        let bytes = self.child_call::<O>(&request, true, retry).await?;
        let id = u64::decode(&bytes).map_err(|error| CallError::Malformed {
            method: O::ID,
            message: error.to_string(),
        })?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    async fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> Result<(), CallError>
    where
        S::Row: Message,
    {
        self.publication::<S>(&[row.encode()], &[self.now()], None).await.map_err(|error| match error {
            super::PublishError::Publication(error) => CallError::Publication(error),
            super::PublishError::Call(error) => error,
        })
    }
    type Error = CallError;
    type Instance<O: OperationDeclaration> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Supervisor::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        Supervisor::with_value(self, value)
    }

    async fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<OperationInstance<O>, CallError> {
        let bytes = self.child_call::<O>(&request, true, None).await?;
        let id = u64::decode(bytes.as_slice()).map_err(|error| CallError::Malformed {
            method: O::ID,
            message: error.to_string(),
        })?;
        Ok(OperationInstance {
            host: self.clone(),
            id: Id::new(id),
            released: false,
            _operation: PhantomData,
        })
    }

    async fn submit_owned<O: OperationDeclaration>(
        &self, _: O::Request, _: harmos_operations::execution::operations::RequestClaims,
    ) -> Result<OperationInstance<O>, CallError> {
        Err(CallError::LocalResources { method: O::ID })
    }

    // Sequence and timestamp each row as the stream publisher does.
}

impl harmos_operations::execution::operations::ChildInstance for Child {
    fn id(&self) -> Id {
        self.id
    }
    fn status(&self) -> harmos_operations::execution::operations::ControlFuture<'_, Status> {
        Box::pin(async move {
            let result = async {
                let bytes = self.host.child_control(self.id, WireChildAction::Status).await?;
                decode_status(bytes.as_slice())
            }
            .await;
            result.map_err(|error: CallError| harmos_operations::execution::operations::InvocationError::failed(self.target.id, error))
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

fn decode_response<O: OperationDeclaration>(bytes: &[u8]) -> Result<Response<O::Response>, CallError> {
    let response = O::Response::decode(bytes).map_err(|error| CallError::Malformed {
        method: O::ID,
        message: error.to_string(),
    })?;
    Ok(Response::new(response))
}

fn decode_status(bytes: &[u8]) -> Result<Status, CallError> {
    let malformed = |message: String| CallError::Malformed {
        method: harmos_operations::execution::operations::CHILD_METHOD,
        message,
    };
    let state = InstanceState::decode(bytes).map_err(|error| malformed(error.to_string()))?;
    Status::try_from(&state).map_err(|error| malformed(error.to_string()))
}

/* === Tests === */
