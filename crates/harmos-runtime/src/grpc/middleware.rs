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
use std::{
    future::{Future, poll_fn},
    net::SocketAddr,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

// Third Party Imports
use tonic::{Code, codegen::http::Extensions, metadata::MetadataMap};

// Local Imports
use super::ConnectionSession;
use crate::operations::{CallContext, ContextMut};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Enter<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/* === Structs === */

pub struct Invocation {
    path: String,
    metadata: MetadataMap,
    extensions: Extensions,
    peer: Option<SocketAddr>,
    context: CallContext,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub code: Code,
}

pub(super) struct Session {
    call: Invocation,
    entries: Arc<[Arc<dyn Middleware>]>,
    entered: usize,
    observed: bool,
}

/* === Enums === */

/* === Traits === */

pub trait Middleware: Send + Sync + 'static {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a>;
    fn observe(&self, _call: &Invocation, _observation: Observation) {}
}

/* === Implementations === */

impl Invocation {
    pub fn connection(&self) -> Option<&ConnectionSession> {
        self.extensions.get()
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn transport(&self) -> &Extensions {
        &self.extensions
    }

    pub fn metadata(&self) -> &MetadataMap {
        &self.metadata
    }

    pub fn peer(&self) -> Option<SocketAddr> {
        self.peer
    }

    pub fn context(&self) -> &CallContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> ContextMut<'_> {
        ContextMut::new(&mut self.context)
    }
}

impl Session {
    pub(super) fn new(
        path: String, metadata: MetadataMap, extensions: Extensions, peer: Option<SocketAddr>, entries: Arc<[Arc<dyn Middleware>]>,
    ) -> Self {
        let mut context = CallContext::default();
        if let Some(connection) = extensions.get::<ConnectionSession>() {
            context.insert(connection.id()).expect("transport owns connection identity");
            context.insert(connection.clone()).expect("transport owns connection session");
        }
        Self {
            call: Invocation {
                path,
                metadata,
                extensions,
                peer,
                context,
            },
            entries,
            entered: 0,
            observed: false,
        }
    }

    pub(super) async fn enter(&mut self) -> Result<()> {
        for entry in self.entries.iter() {
            self.entered += 1;
            let future: Enter<'_> = Box::pin(async { entry.enter(&mut self.call).await });
            guarded(future).await?;
        }
        Ok(())
    }

    pub(super) fn context(&self) -> CallContext {
        self.call.context.clone()
    }

    pub(super) fn observe(&mut self, code: Code) {
        if self.observed {
            return;
        }
        self.observed = true;
        for entry in self.entries[..self.entered].iter().rev() {
            if catch_unwind(AssertUnwindSafe(|| {
                entry.observe(&self.call, Observation { code });
            }))
            .is_err()
            {
                tracing::error!(path = %self.call.path, "gRPC middleware observation panicked");
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.observe(Code::Cancelled);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn guarded(mut future: Enter<'_>) -> Result<()> {
    poll_fn(|context| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result,
        Err(_) => Poll::Ready(Err(Error::InvalidInput {
            field: "gRPC middleware",
            reason: "middleware entry panicked",
        })),
    })
    .await
}

/* === Tests === */
