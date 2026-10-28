/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

use super::{CallContext, OperationGeneration, OperationInstanceId, OperationOutcome};
use crate::operations::OperationDefinition;
use crate::{Error, Result};

pub type Enter<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

pub struct Invocation {
    operation: OperationDefinition,
    id: OperationInstanceId,
    generation: OperationGeneration,
    context: CallContext,
    origin: super::caller::Origin,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub outcome: OperationOutcome,
    pub settled: bool,
}

#[derive(Default)]
pub(super) struct Configuration {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    frozen: bool,
    entries: Vec<Arc<dyn Middleware>>,
    authorizer: Option<Arc<dyn super::authorization::Authorizer>>,
    authorization_frozen: bool,
}

pub(super) struct Session {
    call: Invocation,
    entries: Vec<Arc<dyn Middleware>>,
    entered: usize,
    observed: bool,
    context: Arc<OnceLock<CallContext>>,
}

pub trait Middleware: Send + Sync + 'static {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a>;

    fn observe(&self, _call: &Invocation, _observation: Observation) {}
}

impl Invocation {
    pub fn is_host_call(&self) -> bool {
        self.origin.identity == super::caller::Identity::Host
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub fn caller_scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.origin.scope.as_ref()
    }

    pub fn operation(&self) -> &OperationDefinition {
        &self.operation
    }

    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub fn generation(&self) -> OperationGeneration {
        self.generation
    }

    pub fn context(&self) -> &CallContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> crate::operations::ContextMut<'_> {
        super::ContextMut::new(&mut self.context)
    }
}

impl Configuration {
    pub(super) fn register(&self, middleware: Arc<dyn Middleware>) -> Result<()> {
        let mut state = self.state.lock().expect("middleware configuration lock is healthy");
        if state.frozen {
            return Err(Error::Closing {
                domain: "operation middleware configuration",
            });
        }
        state.entries.push(middleware);
        Ok(())
    }

    pub(super) fn register_authorizer(&self, authorizer: Arc<dyn super::authorization::Authorizer>) -> Result<()> {
        let mut state = self.state.lock().expect("middleware configuration lock is healthy");
        if state.authorization_frozen {
            return Err(Error::Closing {
                domain: "operation authorization configuration",
            });
        }
        if state.authorizer.is_some() {
            return Err(Error::InvalidInput {
                field: "operation authorizer",
                reason: "authorization already has an owner",
            });
        }
        state.authorizer = Some(authorizer);
        Ok(())
    }

    #[cfg(feature = "grpc")]
    pub(super) fn authorizer(&self) -> Option<Arc<dyn super::authorization::Authorizer>> {
        let mut state = self.state.lock().expect("middleware configuration lock is healthy");
        state.authorization_frozen = true;
        state.authorizer.clone()
    }

    pub(super) fn capture(&self) -> Vec<Arc<dyn Middleware>> {
        let mut state = self.state.lock().expect("middleware configuration lock is healthy");
        state.frozen = true;
        state.entries.clone()
    }
}

impl Session {
    pub(super) fn new(
        entries: Vec<Arc<dyn Middleware>>, operation: OperationDefinition, id: OperationInstanceId, generation: OperationGeneration,
        inherited: CallContext, context: Arc<OnceLock<CallContext>>, origin: super::caller::Origin,
    ) -> Self {
        Self {
            call: Invocation {
                operation,
                id,
                generation,
                context: inherited,
                origin,
            },
            entries,
            entered: 0,
            observed: false,
            context,
        }
    }

    pub(super) async fn enter(&mut self) -> Result<()> {
        for entry in &self.entries {
            self.entered += 1;
            let result = guarded(Box::pin(async { entry.enter(&mut self.call).await })).await;
            if let Err(error) = result {
                self.freeze();
                return Err(error);
            }
        }
        self.freeze();
        Ok(())
    }

    fn freeze(&self) {
        let _ = self.context.set(self.call.context.clone());
    }

    pub(super) fn observe(&mut self, observation: Observation) {
        if self.observed {
            return;
        }
        self.observed = true;
        for entry in self.entries[..self.entered].iter().rev() {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                entry.observe(&self.call, observation);
            }))
            .is_err()
            {
                tracing::error!(operation = %self.call.operation.id, "operation middleware observation panicked");
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.observe(Observation {
            outcome: OperationOutcome::Unavailable,
            settled: false,
        });
    }
}

async fn guarded(mut future: Enter<'_>) -> Result<()> {
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result,
        Err(_) => std::task::Poll::Ready(Err(Error::InvalidInput {
            field: "operation middleware",
            reason: "middleware entry panicked",
        })),
    })
    .await
}
