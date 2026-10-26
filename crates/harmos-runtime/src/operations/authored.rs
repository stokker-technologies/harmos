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
use super::{Context, Manager, OperationInstanceId};
use crate::operations::OperationDeclaration;
use crate::supervision::instance::{Instance, Phase};
use crate::{Error, Result, operations::Scope};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Payload<D> = <D as OperationDeclaration>::Response;

/* === Structs === */

struct Physical {
    instance: Box<dyn harmos_sdk::operations::NativeInstance>,
    _request: Option<super::streaming::RequestClaims>,
}
struct Bound<D: OperationDeclaration> {
    physical: Physical,
    cached: Option<super::Response<Payload<D>>>,
    entry: Arc<Entry>,
    operations: Manager,
    id: OperationInstanceId,
    control: Weak<Instance>,
    attempt: usize,
    admission: Admission,
    source: Source,
    // The admitted caller's grant authority; each attempt context carries it.
    grants: Option<Arc<super::manager::GrantAuthority>>,
    preparation: Option<OwnedSemaphorePermit>,
}

struct Owned {
    physical: Physical,
    control: Weak<Instance>,
    operation: String,
    outcome: Option<Arc<Error>>,
    _admission: Admission,
    _preparation: Option<OwnedSemaphorePermit>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Physical {
    async fn finish(&mut self, name: &str, control: &Weak<Instance>) -> Result<()> {
        if let Some(control) = control.upgrade() {
            control.transition(Phase::Stopping);
        }
        guarded(name, self.instance.stop()).await?;
        if let Some(control) = control.upgrade() {
            control.transition(Phase::Finalizing);
        }
        guarded(name, self.instance.cleanup()).await
    }
}
impl<D: OperationDeclaration> Bound<D> {
    fn context<'s>(&self, scope: Scope<'s>, attempt: usize) -> Result<Context<'s>> {
        let mut origin = self.source.check()?;
        self.entry.available_to(&origin)?;
        origin.grants = self.grants.clone();
        Ok(Context {
            id: self.id,
            attempt,
            generation: self.entry.generation,
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

impl<D: OperationDeclaration> Body<super::Response<Payload<D>>> for Bound<D> {
    fn transferred(&self) -> bool {
        self.physical.instance.transferred()
    }

    fn initialize<'s>(&'s mut self, scope: Scope<'s>) -> Attempt<'s, ()> {
        Box::pin(async move {
            let context = self.context(scope, 0)?;
            if self.cached.is_some() {
                return Ok(());
            }
            let context = execution_context(context)?;
            guarded(self.entry.id(), self.physical.instance.initialize(context)).await?;
            Ok(())
        })
    }

    fn run<'s>(&'s mut self, scope: Scope<'s>) -> Attempt<'s, super::Response<Payload<D>>> {
        Box::pin(async move {
            let context = self.context(scope, self.attempt)?;
            if let Some(response) = &self.cached {
                return Ok(response.clone());
            }
            self.attempt = self.attempt.saturating_add(1);
            let context = execution_context(context)?;
            let response = guarded(self.entry.id(), self.physical.instance.run(context)).await?;
            response
                .restore::<D::Response>()
                .ok_or(Error::InvalidInput { field: "operation response", reason: "native response differs from its declaration" })
        })
    }

    fn cleanup_phase(&self) -> Phase {
        Phase::Stopping
    }

    fn classify(&self, error: &Error) -> crate::operations::FailureKind {
        let Error::OperationFailed { source, .. } = error else {
            return crate::operations::FailureKind::Permanent;
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.physical.instance.classify(source.as_ref())))
            .unwrap_or(crate::operations::FailureKind::Permanent)
    }

    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(self.physical.finish(self.entry.id(), &self.control))
    }

    fn retain(self: Box<Self>, outcome: Option<Error>, cause: Arc<Error>) -> (Box<dyn Finalizer>, Option<Error>, Arc<Error>) {
        let outcome = outcome.map(Arc::new);
        let operation = self.entry.id().to_owned();
        let failure = super::finalization::failed(&operation, outcome.clone(), cause.clone());
        let diagnostic = Arc::new(super::finalization::failed(&operation, outcome.clone(), cause));
        let Self { physical, control, admission, preparation, .. } = *self;
        let owner = Owned { physical, control, operation, outcome, _admission: admission, _preparation: preparation };
        (Box::new(owner), Some(failure), diagnostic)
    }
}

impl Finalizer for Owned {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(self.physical.finish(&self.operation, &self.control))
    }

    fn failure(&self, cause: Arc<Error>) -> Arc<Error> {
        Arc::new(super::finalization::failed(&self.operation, self.outcome.clone(), cause))
    }
}

/* === Public Functions === */

pub(super) fn factory<D: OperationDeclaration, P: Send + 'static>(
    operations: &Manager, factory: harmos_sdk::operations::OperationFactory,
) -> Typed<D, P> {
    factory_with(operations, move |request, endpoints| factory.prepare_native(Box::new(request), Box::new(endpoints)))
}

pub(super) fn factory_with<D: OperationDeclaration, P: Send + 'static>(
    operations: &Manager,
    prepare: impl Fn(D::Request, P) -> std::result::Result<Box<dyn harmos_sdk::operations::NativeInstance>, harmos_sdk::ServingError>
    + Send
    + Sync
    + 'static,
) -> Typed<D, P> {
    let registry = Arc::downgrade(&operations.registry);
    let instances = operations.instances.clone();
    let lanes = operations.lanes.clone();
    let preparation_budget = operations.preparation.clone();
    let resident = operations.resident.clone();
    Typed {
        prepare: Box::new(move |(request, endpoints), id, control, entry, admission, source, origin, preparation| {
            let super::binding::Request { value: request, retention, context, key, .. } = request;
            let registry = registry.upgrade().ok_or(Error::Closing { domain: "operations" })?;
            let operations = Manager {
                registry,
                context,
                instances: instances.clone(),
                lanes: lanes.clone(),
                preparation: preparation_budget.clone(),
                resident: resident.clone(),
            };
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if operations.context.get().is_some_and(|context| context.is_empty()) { key.or_else(|| D::key(&request)) } else { None }
            }))
            .map_err(|_| Error::OperationPanicked { operation: entry.id().to_owned() })?;
            if key.is_some() && std::any::TypeId::of::<P>() != std::any::TypeId::of::<()>() {
                return Err(Error::InvalidInput {
                    field: "operation stream cache",
                    reason: "endpoint execution cannot reuse a response without executing its streams",
                });
            }
            let cached = key.as_ref().and_then(|key| operations.lookup::<D>(&entry, key, &origin.identity));
            let bound = Bound::<D> {
                physical: Physical {
                    instance: prepare(request, endpoints)
                        .map_err(|source| Error::OperationFailed { operation: entry.id().to_owned(), source: Arc::new(source) })?,
                    _request: retention,
                },
                cached,
                entry: entry.clone(),
                operations: operations.clone(),
                id,
                control: Arc::downgrade(&control),
                attempt: 0,
                admission,
                source: source.clone(),
                grants: origin.grants.clone(),
                preparation,
            };
            let mut prepared = super::binding::prepared(&entry, id, Box::new(bound));
            let Some(key) = key else { return Ok(prepared) };
            let registry = Arc::downgrade(&operations.registry);
            prepared.completed = Some(Box::new(move |response| {
                if source.check().is_ok()
                    && let Some(registry) = registry.upgrade()
                {
                    registry.insert::<D>(&entry, key.clone(), origin.identity.clone(), response.clone());
                }
            }));
            Ok(prepared)
        }),
    }
}

/* === Private Functions === */

fn execution_context(context: Context<'_>) -> Result<harmos_sdk::OperationContext> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies
        .insert(Arc::new(context.owned()))
        .map_err(|source| Error::OperationFailed { operation: "host context".to_owned(), source: Arc::new(source) })?;
    Ok(harmos_sdk::OperationContext::new(dependencies, context.context().clone()))
}
async fn guarded<O>(operation: &str, future: impl Future<Output = std::result::Result<O, harmos_sdk::ServingError>>) -> Result<O> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result.map(|result| {
            result.map_err(|error| Error::OperationFailed {
                operation: operation.to_owned(),
                source: match error {
                    harmos_sdk::ServingError::Native(source) => source,
                    source => Arc::new(source),
                },
            })
        }),
        Err(_) => std::task::Poll::Ready(Err(Error::OperationPanicked { operation: operation.to_owned() })),
    })
    .await
}

/* === Tests === */
