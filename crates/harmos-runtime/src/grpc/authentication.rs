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

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc};

// Third Party Imports

// Local Imports
use super::middleware::{Enter, Invocation, Middleware};
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Authentication<'a, P> = Pin<Box<dyn Future<Output = Result<P>> + Send + 'a>>;

/* === Structs === */

pub struct Authenticate<A> {
    authenticator: Arc<A>,
}

/* === Enums === */

/* === Traits === */

pub trait Authenticator: Send + Sync + 'static {
    type Principal: Send + Sync + 'static;
    fn authenticate<'a>(&'a self, call: &'a Invocation) -> Authentication<'a, Self::Principal>;
}

/* === Implementations === */

impl<A: Authenticator> Authenticate<A> {
    pub fn new(authenticator: Arc<A>) -> Self {
        Self { authenticator }
    }
}

impl<A: Authenticator> Middleware for Authenticate<A> {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            let principal = self.authenticator.authenticate(call).await?;
            call.context_mut().insert(principal)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
