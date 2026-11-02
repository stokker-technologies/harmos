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
use std::collections::BTreeSet;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread::ThreadId;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Lanes, Managed};
use super::support::Pulse;
use harmos_runtime::Message;
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::sidecar::inventory::publications;
use harmos_runtime::sidecar::{
    Observer, ObserverAttachment, ObserverCaller, ObserverContext, ObserverHost, ObserverPublisher, PublishError, Publisher, Supervisor,
};

/* === Constants === */

static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Settings>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct State {
    owner: Mutex<Option<ThreadId>>,
    bound_on: Mutex<Vec<ThreadId>>,
    resource_address: AtomicUsize,
    opened: AtomicUsize,
    bindings: AtomicUsize,
    initialized: AtomicUsize,
    stopped: AtomicUsize,
    cleaned: AtomicBool,
}

#[derive(Default)]
struct Settings {
    state: Arc<State>,
    panic_first: bool,
}

struct Seed {
    id: usize,
}

struct Bridge {
    resources: Arc<Settings>,
    attachment: Arc<ObserverAttachment<Host>>,
}

struct Host(Publisher<Bridge>);

#[harmos_runtime::stream(id = "custom_host.pulse", window = 8, row = Pulse)]
struct Published;

struct ReadValue;

struct Watch<const N: u64>(Arc<State>);

/* === Enums === */

#[harmos_domain::catalog]
enum Observers {
    First(Watch<0>),
    Second(Watch<1>),
}

/* === Traits === */

/* === Implementations === */

impl Seed {
    fn new(resources: &Arc<Settings>) -> Self {
        let id = NEXT_SEED.fetch_add(1, Ordering::SeqCst);
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(resources)).is_none());
        Self { id }
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

const LIFECYCLE: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

impl harmos_operations::lifecycle::Policy for Bridge {
    const INITIALIZATION_POLICY: OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: OperationPolicy = LIFECYCLE;
}

impl harmos_runtime::Lifecycle for Bridge {
    type Config = usize;
    type Context = Supervisor;
    type Error = harmos_runtime::sidecar::Refusal;

    async fn prepare(id: usize, setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Self::Error> {
        let resources = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|resources| resources.upgrade())
            .ok_or_else(|| harmos_runtime::sidecar::Refusal::of("original fixture resources unavailable"))?;
        let attachment = ObserverAttachment::<Host>::from_catalog::<Observers>(<Self as harmos_domain::plugin::Declared>::STREAMS)?;
        let plugin = Self { resources, attachment };
        setup.resource::<Settings>(plugin.resources.clone());
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        self.resources.state.resource_address.store(Arc::as_ptr(&self.resources) as usize, Ordering::SeqCst);
        *self.resources.state.owner.lock().unwrap() = Some(std::thread::current().id());
        self.resources.state.opened.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        assert_eq!(self.resources.state.initialized.load(Ordering::SeqCst), self.resources.state.stopped.load(Ordering::SeqCst));
        self.resources.state.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "custom_host", requires(harmos_operations::streams::Publish),  observers = Observers, streams(Published))]
impl harmos_runtime::Plugin for Bridge {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[];

    fn attachment(&self) -> Option<Arc<dyn harmos_sdk::plugin::Attachment<Supervisor>>> {
        Some(self.attachment.clone())
    }
}

// The Inert Descriptor Uses The Same Custom Host Projection As The Callback.
impl<const N: u64> harmos_domain::inventory::observers::Declared for Watch<N> {
    const ENTRY: harmos_domain::inventory::observers::Entry =
        <harmos_runtime::sidecar::__private::ObserverInventory<Host> as harmos_runtime::catalog::Member<Self>>::ENTRY;
}

impl ObserverHost for Host {
    fn bind(context: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self {
        let resources = context.resource::<Settings>().unwrap();
        assert_eq!(Arc::as_ptr(&resources) as usize, resources.state.resource_address.load(Ordering::SeqCst));
        resources.state.bound_on.lock().unwrap().push(std::thread::current().id());
        let previous = resources.state.bindings.fetch_add(1, Ordering::SeqCst);
        assert!(!resources.panic_first || previous != 0, "binding refuses one activation");
        Self(supervisor.publisher::<Bridge>())
    }
}

impl ObserverCaller<ReadValue> for Host {
    type Request = u64;
    type Response = u64;
    type Error = PublishError;

    async fn call(&self, ctx: &ObserverContext<'_, Self>, value: u64) -> Result<u64, PublishError> {
        assert_eq!(ctx.resource::<Settings>().unwrap().state.opened.load(Ordering::SeqCst), 1);
        Ok(41 + value)
    }
}

impl ObserverPublisher<Pulse> for Host {
    type Error = PublishError;
    const STREAM: Option<publications::Entry> = Some(publications::Entry::of::<Published>());

    async fn publish(&self, _: &ObserverContext<'_, Self>, value: Pulse) -> Result<(), PublishError> {
        self.0.publish_on::<Published>(&value).await
    }
}

impl<const N: u64> Observer<Host> for Watch<N> {
    type Error = PublishError;
    const ID: &'static str = if N == 0 { "custom.first" } else { "custom.second" };
    const INTERVAL: std::time::Duration = Duration::from_millis(5);
    const STREAMS: &'static [publications::Entry] = &[publications::Entry::of::<Published>()];

    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, PublishError> {
        let resolved = context.resource::<Settings>().unwrap();
        let resources = resolved.as_ref();
        resources.state.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(Self(resources.state.clone()))
    }

    async fn observe(&mut self, ctx: &ObserverContext<'_, Host>) -> Result<(), PublishError> {
        let value = ctx.call(N).await?;
        ctx.publish(Pulse { sample: value }).await
    }

    async fn stop(&mut self) -> Result<(), PublishError> {
        self.0.stopped.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn exercise(panic_first: bool) {
    let resources = Arc::new(Settings { panic_first, ..Settings::default() });
    let state = resources.state.clone();
    let seed = Seed::new(&resources);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Bridge>::new(capacity).unwrap();
    assert_eq!(state.opened.load(Ordering::SeqCst), 0);
    assert!(SEEDS.lock().unwrap().contains_key(&seed.id));
    let mut fixture =
        Managed::attach(source, serde_json::to_vec(&seed.id).unwrap(), Lanes::Open, capacity, vec![], std::convert::identity).await;
    fixture.publication().await.unwrap();
    fixture.initialize().await.unwrap();
    assert!(!SEEDS.lock().unwrap().contains_key(&seed.id));
    fixture.ready().wait_for(|ready| *ready).await.unwrap();
    let stream = fixture.runtime.streams.publication("custom_host.pulse", 1).unwrap();
    let mut rows = stream.watch();
    let values = async {
        let mut values = BTreeSet::new();
        while values.len() < if panic_first { 1 } else { 2 } {
            let item = std::future::poll_fn(|cx| futures_core::Stream::poll_next(Pin::new(&mut rows), cx)).await;
            match item {
                Some(harmos_runtime::streams::Item::Row(row)) => {
                    values.insert(Pulse::decode(&row.value.payload).unwrap().sample);
                }
                Some(_) => {}
                None => panic!("custom host publication closed before producing"),
            }
        }
        values
    }
    .await;
    assert!(values.iter().all(|value| [41, 42].contains(value)));
    assert_eq!(state.opened.load(Ordering::SeqCst), 1);
    assert_eq!(state.resource_address.load(Ordering::SeqCst), Arc::as_ptr(&resources) as usize);
    assert_eq!(state.bindings.load(Ordering::SeqCst), 2);
    assert_eq!(state.initialized.load(Ordering::SeqCst), if panic_first { 1 } else { 2 });
    assert!(*fixture.ready().borrow());
    let owner = state.owner.lock().unwrap().unwrap();
    assert!(state.bound_on.lock().unwrap().iter().all(|thread| *thread != owner));
    let managed_stop = fixture.shutdown().await;
    let runtime_stop = fixture.runtime.shutdown().await;
    let stopped = managed_stop.and(runtime_stop);
    if panic_first {
        let error = stopped.expect_err("observer binding failure reaches root shutdown");
        let mut cause: &(dyn std::error::Error + 'static) = &error;
        loop {
            if cause.to_string().contains("observer host or context factory panicked") {
                break;
            }
            if let Some(harmos_runtime::Error::TaskFailed { source, .. }) = cause.downcast_ref::<harmos_runtime::Error>() {
                cause = source.as_ref();
            } else if let Some(harmos_runtime::Error::PluginLifecycle { source }) = cause.downcast_ref::<harmos_runtime::Error>() {
                cause = source.as_ref();
            } else if let Some(harmos_runtime::plugin::PluginFailure::Cleanup(failures)) =
                cause.downcast_ref::<harmos_runtime::plugin::PluginFailure>()
            {
                assert_eq!(failures.len(), 1);
                cause = &failures[0].error;
            } else {
                cause = cause.source().expect("structured binding failure is retained");
            }
        }
    } else {
        stopped.unwrap();
    }
    assert!(state.cleaned.load(Ordering::SeqCst));
    assert_eq!(state.initialized.load(Ordering::SeqCst), state.stopped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn custom_hosts_bind() {
    exercise(false).await;
}

#[tokio::test]
async fn binding_panic_isolated() {
    exercise(true).await;
}

/* === Tests === */
