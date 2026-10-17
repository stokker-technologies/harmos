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
use tonic::{Code, metadata::MetadataMap};

// Local Imports
use crate::Result;
use crate::operations::CallContext;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Enter<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/* === Structs === */
pub struct Invocation {
    path: String,
    metadata: MetadataMap,
    extensions: tonic::codegen::http::Extensions,
    peer: Option<std::net::SocketAddr>,
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
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn transport(&self) -> &tonic::codegen::http::Extensions {
        &self.extensions
    }

    pub fn metadata(&self) -> &MetadataMap {
        &self.metadata
    }

    pub fn peer(&self) -> Option<std::net::SocketAddr> {
        self.peer
    }

    pub fn context(&self) -> &CallContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> crate::operations::ContextMut<'_> {
        crate::operations::ContextMut::new(&mut self.context)
    }
}

impl Session {
    pub(super) fn new(
        path: String,
        metadata: MetadataMap,
        extensions: tonic::codegen::http::Extensions,
        peer: Option<std::net::SocketAddr>,
        entries: Arc<[Arc<dyn Middleware>]>,
    ) -> Self {
        Self {
            call: Invocation {
                path,
                metadata,
                extensions,
                peer,
                context: CallContext::default(),
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
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(result) => result,
            Err(_) => std::task::Poll::Ready(Err(crate::Error::InvalidInput {
                field: "gRPC middleware",
                reason: "middleware entry panicked",
            })),
        }
    })
    .await
}

/* === Tests === */
