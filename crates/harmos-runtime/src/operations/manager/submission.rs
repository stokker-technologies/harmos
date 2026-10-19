/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl OperationManager {
    pub fn submit<O: OperationDeclaration>(&self, request: O) -> Result<OperationInstance<O>> {
        self.admit::<O, ()>((request.into(), ()), None, &[], &Source::Host, None)
    }

    pub fn submit_endpoints<O: OperationDeclaration, P: Send + 'static>(&self, request: O, endpoints: P) -> Result<OperationInstance<O>> {
        self.admit::<O, P>((request.into(), endpoints), None, &[], &Source::Host, None)
    }

    pub fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self, request: crate::operations::StreamItem<O>, endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>((crate::operations::binding::Request::charged(request)?, endpoints), None, &[], &Source::Host, None)
    }

    pub fn submit_at<O: OperationDeclaration>(&self, id: &str, request: O) -> Result<OperationInstance<O>> {
        self.admit_at::<O, ()>(id, (request.into(), ()), None, &[], &Source::Host, None)
    }

    pub async fn call<O: OperationDeclaration>(&self, request: O) -> Result<crate::operations::Response<O::Response>> {
        self.submit(request)?.await
    }

    pub fn submit_caused<O: OperationDeclaration>(&self, request: O, cause: Position) -> Result<OperationInstance<O>> {
        self.admit::<O, ()>((request.into(), ()), None, &[], &Source::Host, Some(cause))
    }

    pub async fn call_caused<O: OperationDeclaration>(
        &self, request: O, cause: Position,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_caused::<O>(request, cause)?.await
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) fn submit_by<O: OperationDeclaration, P: Send + 'static>(
        &self, input: (crate::operations::binding::Request<O>, P), source: &Source,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(input, None, &[], source, None)
    }

    pub(in crate::operations) fn submit_scoped<O: OperationDeclaration, P: Send + 'static>(
        &self, input: (crate::operations::binding::Request<O>, P), scope: &Scope<'_>, admission: &Admission, source: &Source,
        cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(input, Some(scope), admission.held(), source, cause)
    }

    pub(super) fn admit<O: OperationDeclaration, P: Send + 'static>(
        &self, input: (crate::operations::binding::Request<O>, P), scope: Option<&Scope<'_>>, ancestors: &[&'static str], source: &Source,
        cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        self.admit_at::<O, P>(O::ID, input, scope, ancestors, source, cause)
    }

    pub(in crate::operations) fn admit_at<O: OperationDeclaration, P: Send + 'static>(
        &self, id: &str, input: (crate::operations::binding::Request<O>, P), scope: Option<&Scope<'_>>, ancestors: &[&'static str],
        source: &Source, cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        let submitted = executor::now();
        let origin = source.check()?;
        let entry = self.entry_as(id, &origin)?;
        self.admit_entry::<O, P>(input, scope, ancestors, source, Submission { submitted, entry, cause, preparation: None })
    }

    pub(in crate::operations) fn admit_entry<O: OperationDeclaration, P: Send + 'static>(
        &self, (mut request, endpoints): (crate::operations::binding::Request<O>, P), scope: Option<&Scope<'_>>,
        ancestors: &[&'static str], source: &Source, submission: Submission,
    ) -> Result<OperationInstance<O>> {
        let Submission { submitted, entry, cause, preparation } = submission;
        let origin = source.check()?;
        if self.registry.state.lock().expect("operation registry lock is healthy").closing
            && !entry.retirement_control()
            && !(scope.is_some() && origin.has_lifecycle_loan())
        {
            return Err(Error::Closing { domain: "operations" });
        }
        entry.available_to(&origin)?;
        entry.authorize(&origin)?;
        let authorization = self.authorize_external(&entry, &origin)?;
        let authority = entry.grant_authority(&origin, authorization);
        authority.admit(O::grant(&request.value).as_ref())?;
        // Encoded bindings carry the authority to the provider's prepared grant.
        let origin = Origin { grants: Some(Arc::new(authority)), ..origin };
        // Authoring Callbacks Run Outside Registry/Pending Locks. Recheck The Provider.
        source.check()?;
        entry.available_to(&origin)?;
        let binding = Arc::clone(&entry.binding).downcast::<Typed<O, P>>();
        #[cfg(feature = "plugin-sidecar")]
        let binding = binding.or_else(|original| {
            entry.native_binding.as_ref().map_or(Err(original), |encoded| Arc::clone(encoded).downcast::<Typed<O, P>>())
        });
        let binding = binding.map_err(|_| Error::InvalidInput {
            field: "operation",
            reason: "identity belongs to a different operation or endpoint type",
        })?;
        let admission = self.lanes.admit(ancestors, entry.lane, entry.bounded)?;
        let id = crate::operations::handler::fresh_instance()?;
        let mut pending = entry.pending.lock().expect("operation settlement lock is healthy");
        source.check()?;
        entry.available_to(&origin)?;
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if entry.sdk.is_some() && pending.values().any(|control| !control.is_settled()) {
            return Err(Error::ResourceExhausted { resource: "SDK physical settlement" });
        }
        let privileged = entry.lifecycle_control() || origin.has_lifecycle_loan();
        let owner = Arc::downgrade(&entry);
        let retired = move || {
            if let Some(entry) = owner.upgrade() {
                let removed = entry.pending.lock().expect("operation settlement lock is healthy").remove(&id);
                drop(removed);
            }
        };
        let reservation = match scope {
            Some(scope) => scope.reserve_tracked(id, entry.lane, submitted, privileged, retired)?,
            None => self.instances.reserve(id, entry.lane, submitted, privileged, retired)?,
        };
        let control = reservation.control();
        self.instances.assign(id, origin.identity.clone());
        pending.insert(id, Arc::clone(&control));
        drop(pending);

        let middleware = self.registry.middleware.capture();
        let context = if middleware.is_empty() { self.context.clone() } else { Arc::new(std::sync::OnceLock::new()) };
        request.context = context.clone();
        let mut job = (binding.prepare)(
            (request, endpoints),
            id,
            Arc::clone(&control),
            Arc::clone(&entry),
            admission,
            source.clone(),
            origin.clone(),
            preparation,
        )?;
        #[cfg(feature = "grpc")]
        if let Some(deadline) = self.call_context().get::<crate::operations::context::Deadline>() {
            job.policy.timeout = job.policy.timeout.min(deadline.0.saturating_duration_since(submitted));
        }
        if !middleware.is_empty() {
            job.middleware = Some(crate::operations::middleware::Session::new(
                middleware,
                entry.definition.clone(),
                id,
                entry.generation,
                self.call_context().clone(),
                context,
                origin,
            ));
        }
        let refusal = if executor::now().saturating_duration_since(submitted) >= entry.definition.policy.timeout {
            Err(crate::Refusal::TimedOut.into())
        } else {
            source
                .check()
                .and_then(|origin| entry.available_to(&origin))
                .and_then(|_| if self.instances.is_accepting() { Ok(()) } else { Err(Error::Closing { domain: "operation" }) })
        };
        if let Err(error) = refusal {
            job.refusal = Some(error);
        }
        let outcome = reservation.submit(job, cause);
        Ok(OperationInstance {
            id,
            generation: entry.generation,
            outcome: crate::operations::handler::Outcome::Request(outcome),
            control,
            released: false,
        })
    }
}
