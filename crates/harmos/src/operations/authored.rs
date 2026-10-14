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
use std::future::Future;
use std::sync::{Arc, Weak};

// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::admission::Admission;
use super::binding::Typed;
use super::caller::Source;
use super::execution::{Attempt, Body};
use super::finalization::{Finalize, Finalizer};
use super::manager::Entry;
use super::{Context, Operation, OperationInstanceId, OperationManager};
use crate::operations::OperationDeclaration;
use crate::supervision::instance::{Instance, Phase};
use crate::{Error, Result, operations::Scope};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Payload<T> = <<T as Operation>::Declaration as OperationDeclaration>::Response;

/* === Structs === */
struct Physical<T: Operation> {
    resources: Arc<T::Resources>,
    operation: Option<T>,
    request: <T::Declaration as OperationDeclaration>::Request,
    endpoints: T::Endpoints,
    _request: Option<super::streaming::RequestClaims>,
    stopped: bool,
}

struct Bound<T: Operation> {
    physical: Physical<T>,
    cached: Option<super::Response<Payload<T>>>,
    entry: Arc<Entry>,
    operations: OperationManager,
    id: OperationInstanceId,
    control: Weak<Instance>,
    attempt: usize,
    admission: Admission,
    source: Source,
    preparation: Option<OwnedSemaphorePermit>,
}

struct Owned<T: Operation> {
    physical: Physical<T>,
    control: Weak<Instance>,
    operation: String,
    outcome: Option<Arc<Error>>,
    _admission: Admission,
    _preparation: Option<OwnedSemaphorePermit>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T: Operation> Physical<T> {
    async fn finish(&mut self, name: &str, control: &Weak<Instance>) -> Result<()> {
        // An Operation That Never Initialized Owns Nothing To Stop
        let Some(operation) = self.operation.as_mut() else {
            return Ok(());
        };
        if !self.stopped {
            if let Some(control) = control.upgrade() {
                control.transition(Phase::Stopping);
            }
            guarded(name, async { operation.stop().await }).await?;
            self.stopped = true;
        }
        if let Some(control) = control.upgrade() {
            control.transition(Phase::Finalizing);
        }
        guarded(name, async { operation.cleanup().await }).await
    }
}

impl<T: Operation> Bound<T> {
    fn context<'s>(&self, scope: Scope<'s>, attempt: usize) -> Result<Context<'s>> {
        let origin = self.source.check()?;
        self.entry.available_to(&origin)?;
        Ok(Context {
            id: self.id,
            attempt,
            scope,
            operations: self.operations.clone(),
            admission: self.admission.clone(),
            source: self.source.for_attempt(&self.entry, scope, &origin),
            #[cfg(feature = "plugin-sidecar")]
            retirement: self.entry.retirement_control() || origin.has_lifecycle_loan(),
            origin,
        })
    }
}

impl<T: for<'ctx> Operation<Context<'ctx> = Context<'ctx>>> Body<super::Response<Payload<T>>>
    for Bound<T>
{
    fn transferred(&self) -> bool {
        self.physical
            .operation
            .as_ref()
            .is_some_and(Operation::transferred)
            || super::EndpointSet::transferred(&self.physical.endpoints)
    }

    fn initialize<'s>(&'s mut self, scope: Scope<'s>) -> Attempt<'s, ()> {
        Box::pin(async move {
            let context = self.context(scope, 0)?;
            if self.cached.is_some() {
                return Ok(());
            }
            if self.physical.operation.is_some() {
                return Ok(());
            }
            let resources = Arc::clone(&self.physical.resources);
            let operation = guarded(self.entry.id(), async {
                T::initialize(resources, context).await
            })
            .await?;
            self.physical.operation = Some(operation);
            Ok(())
        })
    }

    fn run<'s>(&'s mut self, scope: Scope<'s>) -> Attempt<'s, super::Response<Payload<T>>> {
        Box::pin(async move {
            let context = self.context(scope, self.attempt)?;
            if let Some(response) = &self.cached {
                return Ok(response.clone());
            }
            self.attempt = self.attempt.saturating_add(1);
            guarded(self.entry.id(), async {
                self.physical
                    .operation
                    .as_mut()
                    .expect("initialized operation")
                    .run(
                        context,
                        &mut self.physical.request,
                        &mut self.physical.endpoints,
                    )
                    .await
            })
            .await
        })
    }

    fn cleanup_phase(&self) -> Phase {
        Phase::Stopping
    }

    fn classify(&self, error: &Error) -> crate::operations::Failure {
        super::binding::classify(error, T::classify)
    }

    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(self.physical.finish(self.entry.id(), &self.control))
    }

    fn retain(
        self: Box<Self>,
        outcome: Option<Error>,
        cause: Arc<Error>,
    ) -> (Box<dyn Finalizer>, Option<Error>, Arc<Error>) {
        let outcome = outcome.map(Arc::new);
        let operation = self.entry.id().to_owned();
        let failure = super::finalization::failed(&operation, outcome.clone(), cause.clone());
        let diagnostic = Arc::new(super::finalization::failed(
            &operation,
            outcome.clone(),
            cause,
        ));
        let Self {
            physical,
            control,
            admission,
            preparation,
            ..
        } = *self;
        let owner = Owned {
            physical,
            control,
            operation,
            outcome,
            _admission: admission,
            _preparation: preparation,
        };
        (Box::new(owner), Some(failure), diagnostic)
    }
}

impl<T: Operation> Finalizer for Owned<T> {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(self.physical.finish(&self.operation, &self.control))
    }

    fn failure(&self, cause: Arc<Error>) -> Arc<Error> {
        Arc::new(super::finalization::failed(
            &self.operation,
            self.outcome.clone(),
            cause,
        ))
    }
}

/* === Public Functions === */
pub(super) fn factory<T: for<'ctx> Operation<Context<'ctx> = Context<'ctx>>>(
    operations: &OperationManager,
    resources: Arc<T::Resources>,
) -> Typed<T::Declaration, T::Endpoints> {
    let registry = Arc::downgrade(&operations.registry);
    let instances = operations.instances.clone();
    let lanes = operations.lanes.clone();
    let preparation_budget = operations.preparation.clone();
    let resident = operations.resident.clone();
    Typed {
        prepare: Box::new(
            move |(request, endpoints),
                  id,
                  control,
                  entry,
                  admission,
                  source,
                  origin,
                  preparation| {
                let super::binding::Request {
                    value: request,
                    retention,
                    context,
                } = request;
                let registry = registry.upgrade().ok_or(Error::Closing {
                    domain: "operations",
                })?;
                let operations = OperationManager {
                    registry,
                    context,
                    instances: instances.clone(),
                    lanes: lanes.clone(),
                    preparation: preparation_budget.clone(),
                    resident: resident.clone(),
                };
                let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if operations
                        .context
                        .get()
                        .is_some_and(|context| context.is_empty())
                    {
                        T::Declaration::key(&request)
                    } else {
                        None
                    }
                }))
                .map_err(|_| Error::OperationPanicked {
                    operation: entry.id().to_owned(),
                })?;
                if key.is_some()
                    && std::any::TypeId::of::<T::Endpoints>() != std::any::TypeId::of::<()>()
                {
                    return Err(Error::InvalidInput {
                        field: "operation stream cache",
                        reason: "endpoint execution cannot reuse a response without executing its streams",
                    });
                }
                let cached = key.as_ref().and_then(|key| {
                    operations.lookup::<T::Declaration>(&entry, key, &origin.identity)
                });
                let bound = Bound::<T> {
                    physical: Physical {
                        resources: resources.clone(),
                        operation: None,
                        request,
                        endpoints,
                        _request: retention,
                        stopped: false,
                    },
                    cached,
                    entry: entry.clone(),
                    operations: operations.clone(),
                    id,
                    control: Arc::downgrade(&control),
                    attempt: 0,
                    admission,
                    source: source.clone(),
                    preparation,
                };
                let mut prepared = super::binding::prepared(&entry, id, Box::new(bound));
                let Some(key) = key else { return Ok(prepared) };
                let registry = Arc::downgrade(&operations.registry);
                prepared.completed = Some(Box::new(move |response| {
                    if source.check().is_ok()
                        && let Some(registry) = registry.upgrade()
                    {
                        registry.insert::<T::Declaration>(
                            &entry,
                            key.clone(),
                            origin.identity.clone(),
                            response.clone(),
                        );
                    }
                }));
                Ok(prepared)
            },
        ),
    }
}

/* === Private Functions === */
async fn guarded<O, E>(
    operation: &str,
    future: impl Future<Output = std::result::Result<O, E>>,
) -> Result<O>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(result) => result.map(|result| {
                result.map_err(|source| Error::OperationFailed {
                    operation: operation.to_owned(),
                    source: Arc::new(source),
                })
            }),
            Err(_) => std::task::Poll::Ready(Err(Error::OperationPanicked {
                operation: operation.to_owned(),
            })),
        }
    })
    .await
}

/* === Tests === */
