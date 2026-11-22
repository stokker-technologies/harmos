/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Child Call Boundary

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{convert::Infallible, marker::PhantomData, num::NonZeroUsize, sync::Arc};

// Third Party Imports

// Local Imports
use super::{ChildTransport, Context};
use crate::{Error, OperationRequest, Result, message};
use harmos_protocol::plugin::ChildRequest;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub(crate) struct Queue {
    pub child: Option<Arc<dyn ChildTransport>>,
}

/// Guest Child Imports Are Unary; Watch Handles Cannot Be Constructed.
#[must_use]
pub struct Watching<R: message::Message> {
    unavailable: Infallible,
    response: PhantomData<fn() -> R>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Context {
    pub(super) async fn send_call(&self, request: ChildRequest) -> Result<Vec<u8>> {
        let transport = self.queue.child.as_ref().ok_or_else(|| Error::Invalid("Call Guest Child: Bind an Attempt Context".into()))?;
        transport.call(request, self.values.clone(), false).await
    }

    /// Guest Imports Admit Unary Operations Only.
    pub async fn watch<Q: OperationRequest>(&self, _: Q, _: NonZeroUsize) -> Result<Watching<Q::Response>> {
        Err(Error::Invalid(format!("Watch {}: Use a Native Plugin Carrier for Streaming Contracts", Q::ID)))
    }
}

impl<R: message::Message> Watching<R> {
    pub fn close(&mut self) {
        match self.unavailable {}
    }

    pub async fn next(&mut self) -> Result<Option<R>> {
        match self.unavailable {}
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
