/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Child Transport

/* === Modules === */

#[cfg(target_arch = "wasm32")]
mod release;
#[cfg(not(target_arch = "wasm32"))]
mod release_native;

/* === Re-Exports === */

/* === Imports === */

use crate::{Context, Error, Message, OperationRequest, Result, context::ChildTransport, operation::LocalFuture};
use prost::Message as _;
use std::{
    marker::PhantomData,
    sync::{Arc, OnceLock},
};

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_protocol::model::InstanceState;
use harmos_protocol::model::OperationPolicy;
use harmos_protocol::plugin::ChildRequest;
use harmos_protocol::plugin::GuestChild;
use harmos_protocol::plugin::GuestChildControl;
use harmos_protocol::plugin::GuestChildRequest;
use harmos_protocol::plugin::GuestSubmitted;
use harmos_protocol::plugin::GuestValue;
use harmos_protocol::plugin::OperationAssignments;
use harmos_protocol::plugin::{GuestChildAction, guest_child};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Import = dyn Fn(Vec<u8>) -> LocalFuture<'static, Result<Vec<u8>>> + Send + Sync;

/* === Structs === */

pub(crate) struct Transport {
    pub import: Arc<Import>,
    pub context: Vec<u8>,
    pub generation_context: Vec<u8>,
    pub assignments: Arc<OnceLock<OperationAssignments>>,
}

/// The original attempt owns the child until join, release or parent settlement.
#[must_use = "dropping a child releases and cancels its work"]
pub struct Child<R: Message> {
    context: Context,
    instance: u64,
    released: bool,
    response: PhantomData<fn() -> R>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ChildTransport for Transport {
    fn call<'a>(&'a self, request: ChildRequest, values: Vec<GuestValue>, submit: bool) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let request = self.request(request, values, submit)?;
            self.exchange(request).await
        })
    }
}

impl Transport {
    pub(super) fn exchange(&self, request: GuestChild) -> LocalFuture<'_, Result<Vec<u8>>> {
        (self.import)(request.encode_to_vec())
    }
    pub fn request(&self, mut request: ChildRequest, values: Vec<GuestValue>, submit: bool) -> Result<GuestChild> {
        let assignment = self
            .assignments
            .get()
            .and_then(|assignments| {
                assignments
                    .operations
                    .iter()
                    .find(|entry| entry.operation.split_once('.').is_some_and(|(_, local)| local == request.operation))
            })
            .ok_or_else(|| Error::Invalid(format!("Unassigned Operation: {}", request.operation)))?;
        if assignment.fingerprint != request.fingerprint {
            return Err(Error::Invalid(format!("Assigned Fingerprint Differs: {}", request.operation)));
        }
        request.operation = assignment.operation.clone();
        Ok(GuestChild {
            action: Some(guest_child::Action::Call(GuestChildRequest {
                context: self.context.clone(),
                generation_context: self.generation_context.clone(),
                request: Some(request),
                generation: assignment.generation,
                submit,
                values,
            })),
        })
    }
}

impl Context {
    pub(super) fn guest_transport(&self) -> Result<&Transport> {
        let transport = self.child.as_ref().ok_or_else(|| Error::Invalid("Guest Attempt Context Required".into()))?;
        (transport.as_ref() as &dyn std::any::Any)
            .downcast_ref::<Transport>()
            .ok_or_else(|| Error::Invalid("Guest Transport Required".into()))
    }

    /// Clone portable protobuf values into a child context without changing the parent.
    pub fn with_value<T: Message>(&self, value: T) -> Self {
        let mut context = self.clone();
        let key = T::name();
        context.values.retain(|entry| entry.key != key);
        context.values.push(GuestValue { key, payload: value.encode_to_vec() });
        context
    }
    pub fn value<T: Message>(&self) -> Result<Option<T>> {
        self.values
            .iter()
            .find(|entry| entry.key == T::name())
            .map(|entry| T::decode(entry.payload.as_slice()).map_err(|error| Error::Invalid(error.to_string())))
            .transpose()
    }
    pub async fn submit<Q: OperationRequest>(&self, request: Q) -> Result<Child<Q::Response>> {
        self.submit_with(request, None).await
    }
    pub async fn submit_with<Q: OperationRequest>(&self, request: Q, policy: Option<OperationPolicy>) -> Result<Child<Q::Response>> {
        let transport = self.guest_transport()?;
        let fingerprint = crate::message::request_fingerprint::<Q>()?;
        if self.requires.get(Q::ID) != Some(&fingerprint) {
            return Err(Error::Invalid(format!("Undeclared Requirement: {}", Q::ID)));
        }
        let bytes = transport
            .call(
                ChildRequest {
                    call: 0,
                    operation: Q::ID.into(),
                    payload: request.encode_to_vec(),
                    fingerprint,
                    policy,
                    ..Default::default()
                },
                self.values.clone(),
                true,
            )
            .await?;
        let admitted = GuestSubmitted::decode(bytes.as_slice()).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Child {
            context: self.clone(),
            instance: admitted.instance,
            released: false,
            response: PhantomData,
        })
    }
    async fn child_control(&self, instance: u64, action: GuestChildAction) -> Result<Vec<u8>> {
        let transport = self.guest_transport()?;
        let request = GuestChild {
            action: Some(guest_child::Action::Control(GuestChildControl {
                context: transport.context.clone(),
                instance,
                action: action as i32,
            })),
        };
        transport.exchange(request).await
    }
}

impl<R: Message> Child<R> {
    pub fn id(&self) -> u64 {
        self.instance
    }
    pub async fn status(&self) -> Result<InstanceState> {
        let bytes = self.context.child_control(self.instance, GuestChildAction::Status).await?;
        InstanceState::decode(bytes.as_slice()).map_err(|error| Error::Invalid(error.to_string()))
    }
    pub async fn request_cancel(&self) -> Result<()> {
        self.context.child_control(self.instance, GuestChildAction::Cancel).await?;
        Ok(())
    }
    pub async fn join(mut self) -> Result<R> {
        let answer = self.context.child_control(self.instance, GuestChildAction::Join).await;
        self.released = true;
        R::decode(answer?.as_slice()).map_err(|error| Error::Invalid(error.to_string()))
    }
    pub async fn cancel(self) -> Result<R> {
        self.request_cancel().await?;
        self.join().await
    }
    pub async fn release(mut self) -> Result<()> {
        self.context.child_control(self.instance, GuestChildAction::Release).await?;
        self.released = true;
        Ok(())
    }
    pub async fn detach(mut self) -> Result<u64> {
        self.context.child_control(self.instance, GuestChildAction::Detach).await?;
        self.released = true;
        Ok(self.instance)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
