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

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports
use tokio::sync::oneshot;
// Local Imports
use super::{Activity, Instance, ObserverError, ObserverHost};
use crate::{Refusal, Supervisor};
use harmos_contract::observers::Control;
use harmos_contract::operations::{Operation, ServingError, Visibility, WireOperation, WireTable};
use harmos_contract::plugin::Attachment;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ObserverAttachment<R, H = Supervisor> {
    resources: Arc<R>,
    declarations: Vec<harmos_contract::inventory::observers::Declaration>,
    instances: BTreeMap<&'static str, Instance<R, H>>,
    closing: AtomicBool,
}

struct Observe<R, H, O> {
    attachment: Arc<ObserverAttachment<R, H>>,
    running: Option<Running>,
    activity: Option<Activity>,
    _operation: PhantomData<fn() -> O>,
}

struct Interrupt<'a>(&'a mut Option<oneshot::Sender<()>>);

// One activation observing on its own thread until told to stop; joining it
// hands the activity back for stopping and cleanup.
struct Running {
    stop: Option<oneshot::Sender<()>>,
    failed: Option<oneshot::Receiver<ObserverError>>,
    returned: Option<oneshot::Receiver<Activity>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<R: Send + Sync + 'static, H: ObserverHost<R>> ObserverAttachment<R, H> {
    pub fn new(
        instances: Vec<Instance<R, H>>,
        resources: Arc<R>,
        grants: &[harmos_contract::inventory::publications::Entry],
    ) -> Result<Arc<Self>, Refusal> {
        super::validate_outputs(&instances, grants).map_err(Refusal::of)?;
        let declarations: Vec<_> = instances.iter().map(Instance::declaration).collect();
        let mut selected = BTreeMap::new();
        for instance in instances {
            if instance.id().is_empty() || selected.contains_key(instance.id()) {
                return Err(Refusal::of(
                    "observer identities must be nonempty and unique",
                ));
            }
            selected.insert(instance.id(), instance);
        }
        Ok(Arc::new(Self {
            resources,
            declarations,
            instances: selected,
            closing: AtomicBool::new(false),
        }))
    }

    pub fn from_catalog<C>(
        resources: Arc<R>,
        grants: &[harmos_contract::inventory::publications::Entry],
    ) -> Result<Arc<Self>, Refusal>
    where
        C: harmos_contract::catalog::Catalog<super::Declarations<R, H>>,
    {
        use harmos_contract::catalog::Projection;
        let mut instances = Vec::new();
        C::ENTRIES
            .try_for_each(|prepare| {
                instances.push(prepare());
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();
        Self::new(instances, resources, grants)
    }

    // Bind the plugin's observe control, the one Host operation the host
    // submits to run an observer.
    pub fn bind<O>(self: &Arc<Self>, table: &mut WireTable<Supervisor>) -> Result<(), ServingError>
    where
        O: WireOperation<Request = Control, Response = ()>,
    {
        if O::VISIBILITY != Visibility::Host || O::POLICY != harmos_contract::observers::POLICY {
            return Err(ServingError::Catalog);
        }
        table.bind_resident_operation::<Observe<R, H, O>>(Arc::clone(self))
    }
}

impl<R: Send + Sync + 'static, H: ObserverHost<R>> Attachment<Supervisor>
    for ObserverAttachment<R, H>
{
    fn observers(&self) -> Vec<harmos_contract::inventory::observers::Declaration> {
        self.declarations.clone()
    }

    fn resources(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        self.resources.clone()
    }

    // Running activations are invocations the host cancels and cleans up;
    // quiescing only refuses new ones.
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>> {
        self.closing.store(true, Ordering::Release);
        Box::pin(std::future::ready(Ok(())))
    }
}

impl<R, H, O> Operation for Observe<R, H, O>
where
    R: Send + Sync + 'static,
    H: ObserverHost<R>,
    O: WireOperation<Request = Control, Response = ()>,
{
    type Declaration = O;
    type Resources = ObserverAttachment<R, H>;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;

    async fn initialize<'ctx>(
        attachment: Arc<Self::Resources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            attachment,
            running: None,
            activity: None,
            _operation: PhantomData,
        })
    }

    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        control: &mut Control,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<()>, Refusal> {
        if self.running.is_none() {
            if self.attachment.closing.load(Ordering::Acquire) {
                return Err(Refusal::of("observer admission is closing"));
            }
            let name = self
                .attachment
                .instances
                .get(control.observer.as_str())
                .ok_or_else(|| Refusal::of("unknown observer"))?
                .id();
            self.running = Some(Running::spawn(
                Arc::clone(&self.attachment),
                name,
                host.clone(),
            )?);
        }
        let running = self.running.as_mut().expect("spawned activation");
        let failed = running.failed.take();
        // Cancelling This Attempt Interrupts The Observing Thread
        let _interrupt = Interrupt(&mut running.stop);
        let failure = match failed {
            Some(failed) => failed.await.ok(),
            None => None,
        };
        match failure {
            Some(failure) => Err(Refusal::of(failure)),
            None => Err(Refusal::of("observer activation ended")),
        }
    }

    async fn stop(&mut self) -> Result<(), Refusal> {
        if let Some(mut running) = self.running.take()
            && let Some(activity) = running.join().await?
        {
            self.activity = Some(activity);
        }
        match self.activity.as_mut() {
            Some(activity) => activity.stop().await.map_err(Refusal::of),
            None => Ok(()),
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
    fn spawn<R: Send + Sync + 'static, H: ObserverHost<R>>(
        attachment: Arc<ObserverAttachment<R, H>>,
        observer: &'static str,
        host: Supervisor,
    ) -> Result<Self, Refusal> {
        let (stop, stopped) = oneshot::channel();
        let (failing, failed) = oneshot::channel();
        let (returning, returned) = oneshot::channel();
        crate::kit::task::spawn(observer.to_owned(), async move {
            let Some(instance) = attachment.instances.get(observer) else {
                return;
            };
            let resources = Arc::clone(&attachment.resources);
            // Binding Happens On The Activation's Own Thread
            let bound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                H::bind(Arc::clone(&resources), host)
            }));
            let mut activity = match bound
                .map_err(|_| ObserverError::BindingPanicked)
                .and_then(|bound| instance.activate(resources, bound))
            {
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
    async fn join(&mut self) -> Result<Option<Activity>, Refusal> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        match self.returned.take() {
            Some(returned) => Ok(returned.await.ok()),
            None => Ok(None),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
