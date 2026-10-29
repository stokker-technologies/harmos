/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{future::Future, pin::Pin, sync::Arc};

use super::middleware::{Enter, Invocation, Middleware};
use crate::Result;

pub type Authentication<'a, P> = Pin<Box<dyn Future<Output = Result<P>> + Send + 'a>>;

pub struct Authenticate<A> {
    authenticator: Arc<A>,
}

pub trait Authenticator: Send + Sync + 'static {
    type Principal: Send + Sync + 'static;
    fn authenticate<'a>(&'a self, call: &'a Invocation) -> Authentication<'a, Self::Principal>;
}

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
