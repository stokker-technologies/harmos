/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Sidecar binding of the portable host capability over the connection frames.
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
use harmos_contract::builtin::InstanceState;
use harmos_contract::operations::instance::{Id, Status};
use harmos_contract::operations::{
    CallContext, Callable, ContextError, ContextValue, Host, Instance, OperationDeclaration,
    Response, WireChildAction,
};
use harmos_contract::{Message, StreamSpec};

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<O: Callable> OperationInstance<O> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub async fn status(&self) -> Result<Status, CallError> {
        let bytes = self
            .host
            .child_control(self.id, WireChildAction::Status)
            .await?;
        decode_status(&bytes)
    }

    pub async fn cancel(&self) -> Result<(), CallError> {
        self.host
            .child_control(self.id, WireChildAction::Cancel)
            .await?;
        Ok(())
    }

    pub async fn join(mut self) -> Result<Response<O::Response>, CallError> {
        let joined = self
            .host
            .child_control(self.id, WireChildAction::Join)
            .await;
        self.released = true;
        let bytes = joined?;
        decode_response::<O>(&bytes)
    }

    pub async fn cancel_and_join(self) -> Result<Response<O::Response>, CallError> {
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
    type Error = CallError;

    fn id(&self) -> Id {
        self.id
    }

    fn status(&self) -> impl Future<Output = Result<Status, CallError>> + Send {
        OperationInstance::status(self)
    }

    fn cancel(&self) -> impl Future<Output = Result<(), CallError>> + Send {
        OperationInstance::cancel(self)
    }

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, CallError>> + Send {
        OperationInstance::join(self)
    }

    fn detach(self) -> Id {
        OperationInstance::detach(self)
    }
}

impl Host for Supervisor {
    type Error = CallError;
    type Instance<O: Callable> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Supervisor::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        Supervisor::with_value(self, value)
    }

    async fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> Result<OperationInstance<O>, CallError> {
        let bytes = self.child_call::<O>(&request, true).await?;
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

    // Resident resources live in the host process; a sidecar cannot lend them.
    async fn submit_with_resources<O: Callable<Response = ()>, R: Send + Sync + 'static>(
        &self,
        _: O::Request,
        _: std::sync::Arc<R>,
    ) -> Result<OperationInstance<O>, CallError> {
        Err(CallError::LocalResources { method: O::ID })
    }

    async fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> Result<Response<O::Response>, CallError> {
        let bytes = self.child_call::<O>(&request, false).await?;
        decode_response::<O>(&bytes)
    }

    // Sequence and timestamp each row as the stream publisher does.
    async fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> Result<(), CallError>
    where
        S::Row: Message,
    {
        self.publication::<S>(row, None)
            .await
            .map_err(|error| match error {
                super::PublishError::Publication(error) => CallError::Publication(error),
                super::PublishError::Call(error) => error,
            })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn decode_response<O: OperationDeclaration>(
    bytes: &[u8],
) -> Result<Response<O::Response>, CallError>
where
    O::Response: Message,
{
    let response = O::Response::decode(bytes).map_err(|error| CallError::Malformed {
        method: O::ID,
        message: error.to_string(),
    })?;
    Ok(Response::new(response))
}

fn decode_status(bytes: &[u8]) -> Result<Status, CallError> {
    let malformed = |message: String| CallError::Malformed {
        method: harmos_contract::operations::CHILD_METHOD,
        message,
    };
    let state = InstanceState::decode(bytes).map_err(|error| malformed(error.to_string()))?;
    Status::try_from(&state).map_err(|error| malformed(error.to_string()))
}

/* === Tests === */
