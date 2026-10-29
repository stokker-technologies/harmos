/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serves a plugin's observers through its one observe control. Each host
// submission names an observer and runs it for as long as the invocation
// lasts: the attempt binds, initializes and ticks the observer on a thread of
// its own, and the invocation's cleanup stops and cleans it up once the host
// cancels it.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tokio::sync::oneshot;

use super::{Activity, Instance, ObserverError, ObserverHost};
use crate::{Refusal, Supervisor};
use harmos_domain::observers::Control;
use harmos_sdk::__private::{RegistrationBatch, WireTable};
use harmos_sdk::operations::{Carrier, ExecutionContext, ServingError};
use harmos_sdk::plugin::Attachment;

pub struct ObserverAttachment<H = Supervisor> {
    resources: OnceLock<Arc<ExecutionContext>>,
    declarations: Vec<harmos_domain::inventory::observers::Declaration>,
    instances: BTreeMap<&'static str, Instance<H>>,
    closing: AtomicBool,
}

struct Observe<H> {
    attachment: Arc<ObserverAttachment<H>>,
    running: Option<Running>,
    activity: Option<Activity>,
}

struct Interrupt<'a>(&'a mut Option<oneshot::Sender<()>>);

// One activation observing on its own thread until told to stop; joining it
// hands the activity back for stopping and cleanup.
struct Running {
    stop: Option<oneshot::Sender<()>>,
    failed: Option<oneshot::Receiver<ObserverError>>,
    returned: Option<oneshot::Receiver<Activity>>,
}

impl<H: ObserverHost> ObserverAttachment<H> {
    pub fn new(instances: Vec<Instance<H>>, grants: &[harmos_domain::inventory::publications::Entry]) -> Result<Arc<Self>, Refusal> {
        super::validate_outputs(&instances, grants).map_err(Refusal::of)?;
        let declarations: Vec<_> = instances.iter().map(Instance::declaration).collect();
        let mut selected = BTreeMap::new();
        for instance in instances {
            if instance.id().is_empty() || selected.contains_key(instance.id()) {
                return Err(Refusal::of("observer identities must be nonempty and unique"));
            }
            selected.insert(instance.id(), instance);
        }
        Ok(Arc::new(Self {
            resources: OnceLock::new(),
            declarations,
            instances: selected,
            closing: AtomicBool::new(false),
        }))
    }

    pub fn from_catalog<C>(grants: &[harmos_domain::inventory::publications::Entry]) -> Result<Arc<Self>, Refusal>
    where
        C: harmos_domain::catalog::Catalog<super::Declarations<H>>,
    {
        use harmos_domain::catalog::Projection;
        let mut instances = Vec::new();
        C::ENTRIES
            .try_for_each(|prepare| {
                instances.push(prepare());
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();
        Self::new(instances, grants)
    }
}

impl<H: ObserverHost> Attachment<Supervisor> for ObserverAttachment<H> {
    // Bind The Built-In Observe Control Before Initialization
    fn bind(self: Arc<Self>, table: &mut WireTable) -> Result<(), ServingError> {
        self.resources.set(Arc::new(table.context())).map_err(|_| ServingError::Catalog)?;
        table.provide_resource(Arc::clone(&self)).map_err(|_| ServingError::Catalog)?;
        table.register(RegistrationBatch::new().carrier::<Control, Observe<H>>())
    }
    fn observers(&self) -> Vec<harmos_domain::inventory::observers::Declaration> {
        self.declarations.clone()
    }

    // Running activations are invocations the host cancels and cleans up;
    // quiescing only refuses new ones.
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>> {
        self.closing.store(true, Ordering::Release);
        Box::pin(std::future::ready(Ok(())))
    }
}

impl<H> Carrier for Observe<H>
where
    H: ObserverHost,
{
    type Declaration = Control;
    type Error = Refusal;
    type StreamEndpoints = ();

    async fn bind(context: ExecutionContext) -> Result<Self, Refusal> {
        let attachment = context.resource::<ObserverAttachment<H>>().map_err(Refusal::of)?;
        Ok(Self {
            attachment,
            running: None,
            activity: None,
        })
    }

    async fn attempt(
        &mut self, context: ExecutionContext, control: &mut <Self::Declaration as harmos_domain::OperationDeclaration>::Request,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<crate::operations::Response<()>, Refusal> {
        if self.running.is_none() {
            if self.attachment.closing.load(Ordering::Acquire) {
                return Err(Refusal::of("observer admission is closing"));
            }
            let name = self.attachment.instances.get(control.observer.as_str()).ok_or_else(|| Refusal::of("unknown observer"))?.id();
            self.running = Some(Running::spawn(
                Arc::clone(&self.attachment),
                name,
                context.resource::<Supervisor>().map_err(Refusal::of)?.as_ref().clone(),
            )?);
        }
        let running = self.running.as_mut().expect("spawned activation");
        let failed = running.failed.as_mut();
        // Cancelling This Attempt Interrupts The Observing Thread
        let _interrupt = Interrupt(&mut running.stop);
        let failure = match failed {
            Some(failed) => failed.await.ok(),
            None => None,
        };
        running.failed.take();
        match failure {
            Some(failure) => Err(Refusal::of(failure)),
            None => Err(Refusal::of("observer activation ended")),
        }
    }

    async fn stop(&mut self) -> Result<(), Refusal> {
        let failure = if let Some(mut running) = self.running.take() {
            let (activity, failure) = running.join().await;
            self.activity = activity;
            failure
        } else {
            None
        };
        let stopped = match self.activity.as_mut() {
            Some(activity) => activity.stop().await.map_err(Refusal::of),
            None => Ok(()),
        };
        match failure {
            Some(failure) => Err(Refusal::of(failure)),
            None => stopped,
        }
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        match self.activity.as_mut() {
            Some(activity) => activity.cleanup().await.map_err(Refusal::of),
            None => Ok(()),
        }
    }
}

impl Drop for Interrupt<'_> {
    fn drop(&mut self) {
        if let Some(stop) = self.0.take() {
            let _ = stop.send(());
        }
    }
}

impl Running {
    fn spawn<H: ObserverHost>(attachment: Arc<ObserverAttachment<H>>, observer: &'static str, host: Supervisor) -> Result<Self, Refusal> {
        let (stop, stopped) = oneshot::channel();
        let (failing, failed) = oneshot::channel();
        let (returning, returned) = oneshot::channel();
        crate::kit::task::spawn(observer.to_owned(), async move {
            let Some(instance) = attachment.instances.get(observer) else {
                return;
            };
            let resources = Arc::clone(attachment.resources.get().expect("observer attachment bound before activation"));
            // Binding Happens On The Activation's Own Thread
            let bound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| H::bind(Arc::clone(&resources), host)));
            let activated = bound.map_err(|_| ObserverError::BindingPanicked).and_then(|bound| instance.activate(resources, bound));
            // The Activity Owns The Activation Now. Release this thread's
            // extra attachment before waking either completion receiver.
            drop(attachment);
            let mut activity = match activated {
                Ok(activity) => activity,
                Err(failure) => {
                    let _ = failing.send(failure);
                    return;
                }
            };
            tokio::select! {
                failure = activity.observe(|| {}) => {
                    let _ = failing.send(failure);
                }
                _ = stopped => {}
            }
            let _ = returning.send(activity);
        })
        .map_err(Refusal::of)?;
        Ok(Self {
            stop: Some(stop),
            failed: Some(failed),
            returned: Some(returned),
        })
    }

    // Interrupt the activation and take its activity back.
    async fn join(&mut self) -> (Option<Activity>, Option<ObserverError>) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let activity = match self.returned.take() {
            Some(returned) => returned.await.ok(),
            None => None,
        };
        // Joining the activation also settles its failure sender. Cancellation
        // of run must not discard an error that shutdown still needs to report.
        let failure = self.failed.take().and_then(|mut failed| failed.try_recv().ok());
        (activity, failure)
    }
}
