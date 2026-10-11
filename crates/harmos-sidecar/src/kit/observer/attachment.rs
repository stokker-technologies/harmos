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
use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
// Third Party Imports

// Local Imports
use super::{Controlled, Instance, ObserverHost, ObserverStatus, control::Stopped};
use crate::{ObserverControl, Refusal, Supervisor};
use harmos_contract::observers::{Control, Status};
use harmos_contract::operations::{
    Failure, Operation, ServingError, Visibility, WireOperation, WireTable,
};
use harmos_contract::plugin::Attachment;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type StopFuture = Pin<Box<dyn Future<Output = Result<Status, Refusal>> + Send>>;

/* === Structs === */
pub struct ObserverAttachment<R, H = Supervisor> {
    resources: Arc<R>,
    declarations: Vec<harmos_contract::inventory::observers::Declaration>,
    state: Mutex<State<R>>,
    _host: PhantomData<fn() -> H>,
}

struct State<R> {
    control: Controlled<Arc<R>>,
    contexts: BTreeMap<String, (u64, [u8; 32])>,
    closing: bool,
}

struct Start<R, H, O> {
    attachment: Arc<ObserverAttachment<R, H>>,
    _operation: PhantomData<fn() -> O>,
}

struct Stop<R, H, O> {
    attachment: Arc<ObserverAttachment<R, H>>,
    work: Option<StopFuture>,
    result: Option<Status>,
    _operation: PhantomData<fn() -> O>,
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
        let mut ids = std::collections::BTreeSet::new();
        if declarations
            .iter()
            .any(|entry| entry.id.is_empty() || !ids.insert(entry.id.clone()))
        {
            return Err(Refusal::of(
                "observer identities must be nonempty and unique",
            ));
        }
        Ok(Arc::new(Self {
            resources,
            declarations,
            _host: PhantomData,
            state: Mutex::new(State {
                control: Controlled::of(instances, Arc::clone),
                contexts: BTreeMap::new(),
                closing: false,
            }),
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

    pub fn bind<A, B>(
        self: &Arc<Self>,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), ServingError>
    where
        A: WireOperation<Request = Control, Response = ()>,
        B: WireOperation<Request = Control, Response = Status>,
    {
        if A::VISIBILITY != Visibility::Host || B::VISIBILITY != Visibility::Host {
            return Err(ServingError::Catalog);
        }
        table.bind_operation::<Start<R, H, A>>(Arc::clone(self))?;
        table.bind_operation::<Stop<R, H, B>>(Arc::clone(self))
    }

    fn begin_stop(&self, control: &Control) -> Result<Stopped, Refusal> {
        let token = token(control)?;
        let mut state = self.state.lock().map_err(Refusal::of)?;
        validate_context(&state.contexts, control, token)?;
        let stopped = state.control.begin_stop(&native(control))?;
        if state
            .contexts
            .get(&control.observer)
            .is_none_or(|(generation, _)| *generation <= control.generation)
        {
            state
                .contexts
                .insert(control.observer.clone(), (control.generation, token));
        }
        Ok(stopped)
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

    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>> {
        let stopped = self
            .state
            .lock()
            .map(|mut state| {
                state.closing = true;
                state.control.begin_quiesce()
            })
            .map_err(|_| ServingError::Catalog);
        Box::pin(async move {
            let stopped = stopped?;
            for stop in stopped {
                let result = stop.wait().await.and_then(|status| match status {
                    ObserverStatus::Stopped(_) => Ok(()),
                    ObserverStatus::Retained(exit) => Err(Refusal::of(exit)),
                    _ => Err(Refusal::of("observer cleanup is pending")),
                });
                result.map_err(|source| ServingError::Handler {
                    failure: Failure::Permanent,
                    source: Arc::new(source),
                })?;
            }
            Ok(())
        })
    }
}

impl<R, H, O> Operation for Start<R, H, O>
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

    fn new(attachment: Arc<Self::Resources>) -> Result<Self, Refusal> {
        Ok(Self {
            attachment,
            _operation: PhantomData,
        })
    }

    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        control: &mut Control,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<(), Refusal> {
        let token = token(control)?;
        let host = host.for_observer_context(token)?;
        let mut state = self.attachment.state.lock().map_err(Refusal::of)?;
        if state.closing {
            return Err(Refusal::of("observer admission is closing"));
        }
        validate_context(&state.contexts, control, token)?;
        state
            .control
            .start(&native(control), &self.attachment.resources, &host)?;
        state
            .contexts
            .insert(control.observer.clone(), (control.generation, token));
        Ok(())
    }
}

impl<R, H, O> Operation for Stop<R, H, O>
where
    R: Send + Sync + 'static,
    H: ObserverHost<R>,
    O: WireOperation<Request = Control, Response = Status>,
{
    type Declaration = O;
    type Resources = ObserverAttachment<R, H>;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;

    fn new(attachment: Arc<Self::Resources>) -> Result<Self, Refusal> {
        Ok(Self {
            attachment,
            work: None,
            result: None,
            _operation: PhantomData,
        })
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        control: &mut Control,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Status, Refusal> {
        if self.work.is_none() && self.result.is_none() {
            token(control)?;
            let stopped = self.attachment.begin_stop(control)?;
            self.work = Some(Box::pin(async move { status(stopped.wait().await?) }));
        }
        self.settle().await
    }

    async fn stop(&mut self) -> Result<(), Refusal> {
        if self.work.is_none() && self.result.is_none() {
            return Ok(());
        }
        self.settle().await.map(|_| ())
    }
}

impl<R: Send + Sync + 'static, H: ObserverHost<R>, O> Stop<R, H, O> {
    async fn settle(&mut self) -> Result<Status, Refusal> {
        if let Some(result) = self.result {
            return Ok(result);
        }
        let result = self
            .work
            .as_mut()
            .expect("original stop future exists")
            .await;
        self.work = None;
        let result = result?;
        self.result = Some(result);
        Ok(result)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn token(control: &Control) -> Result<[u8; 32], Refusal> {
    let token: [u8; 32] = control.context.as_slice().try_into().map_err(Refusal::of)?;
    if control.generation == 0 || token == [0; 32] {
        return Err(Refusal::of("invalid observer generation context"));
    }
    Ok(token)
}

fn validate_context(
    contexts: &BTreeMap<String, (u64, [u8; 32])>,
    control: &Control,
    token: [u8; 32],
) -> Result<(), Refusal> {
    if contexts
        .get(&control.observer)
        .is_some_and(|(generation, prior)| *generation == control.generation && *prior != token)
    {
        return Err(Refusal::of(
            "observer generation context cannot be replaced",
        ));
    }
    Ok(())
}

fn native(control: &Control) -> ObserverControl {
    ObserverControl {
        observer: control.observer.clone(),
        generation: control.generation,
    }
}

fn status(status: ObserverStatus) -> Result<Status, Refusal> {
    let (exit, settled) = match status {
        ObserverStatus::Stopped(exit) => (exit, true),
        ObserverStatus::Retained(exit) => (exit, false),
        _ => return Err(Refusal::of("observer cleanup has not produced a receipt")),
    };
    Ok(Status {
        exit: exit as i32,
        settled,
    })
}

/* === Tests === */
