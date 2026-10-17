/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

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
use super::sidecar::operations::{Lanes, Managed};
use super::support::Pulse;
use harmos::Message;
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::sidecar::inventory::publications;
use harmos::sidecar::{
    Observer, ObserverAttachment, ObserverCaller, ObserverContext, ObserverHost, ObserverPublisher,
    PublishError, Publisher, Supervisor,
};

/* === Constants === */
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Settings>>> =
    Mutex::new(std::collections::BTreeMap::new());

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
    attachment: Arc<ObserverAttachment<Settings, Host>>,
}


struct Host(Publisher<Bridge>);

#[harmos::stream(id = "custom-host.pulse", window = 8, row = Pulse)]
struct Published;

struct ReadValue;

struct Watch<const N: u64>(Arc<State>);

/* === Enums === */
#[harmos::catalog]
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

const LIFECYCLE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

impl harmos::operations::lifecycle::Policy for Bridge {
    const INITIALIZATION_POLICY: OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: OperationPolicy = LIFECYCLE;
}

#[harmos::plugin(id = "custom-host", provides = (), requires(harmos::operations::streams::Publish),  observers = Observers, streams(Published))]
impl harmos::Plugin for Bridge {
    type Config = usize;
    type Resources = Settings;
    type Context = Supervisor;
    type Error = harmos::sidecar::Refusal;
    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let resources = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|resources| resources.upgrade())
            .ok_or_else(|| {
                harmos::sidecar::Refusal::of("original fixture resources unavailable")
            })?;
        let attachment = ObserverAttachment::<Settings, Host>::from_catalog::<Observers>(
            resources.clone(),
            <Self as harmos_domain::plugin::Declared>::STREAMS,
        )?;
        Ok(Self {
            resources,
            attachment,
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        self.resources
            .state
            .resource_address
            .store(Arc::as_ptr(&self.resources) as usize, Ordering::SeqCst);
        *self.resources.state.owner.lock().unwrap() = Some(std::thread::current().id());
        self.resources.state.opened.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn resources(&self) -> Arc<Settings> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos::operations::WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        self.attachment.bind(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_domain::plugin::Attachment<Supervisor>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        assert_eq!(
            self.resources.state.initialized.load(Ordering::SeqCst),
            self.resources.state.stopped.load(Ordering::SeqCst)
        );
        self.resources.state.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

// The Inert Descriptor Uses The Same Custom Host Projection As The Callback.
impl<const N: u64> harmos_domain::inventory::observers::Declared for Watch<N> {
    const ENTRY: harmos_domain::inventory::observers::Entry =
        <harmos::sidecar::__private::ObserverInventory<Host> as harmos::catalog::Member<
            Self,
        >>::ENTRY;
}

impl ObserverHost<Settings> for Host {
    fn bind(resources: Arc<Settings>, supervisor: Supervisor) -> Self {
        assert_eq!(
            Arc::as_ptr(&resources) as usize,
            resources.state.resource_address.load(Ordering::SeqCst)
        );
        resources
            .state
            .bound_on
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        let previous = resources.state.bindings.fetch_add(1, Ordering::SeqCst);
        assert!(
            !resources.panic_first || previous != 0,
            "binding refuses one activation"
        );
        Self(supervisor.publisher::<Bridge>())
    }
}

impl ObserverCaller<Settings, ReadValue> for Host {
    type Request = u64;
    type Response = u64;
    type Error = PublishError;

    async fn call(
        &self,
        ctx: &ObserverContext<'_, Settings, Self>,
        value: u64,
    ) -> Result<u64, PublishError> {
        assert_eq!(ctx.resources().state.opened.load(Ordering::SeqCst), 1);
        Ok(41 + value)
    }
}

impl ObserverPublisher<Settings, Pulse> for Host {
    type Error = PublishError;
    const STREAM: Option<publications::Entry> = Some(publications::Entry::of::<Published>());

    async fn publish(
        &self,
        _: &ObserverContext<'_, Settings, Self>,
        value: Pulse,
    ) -> Result<(), PublishError> {
        self.0.publish_on::<Published>(&value).await
    }
}

impl<const N: u64> Observer<Host> for Watch<N> {
    type Resources = Settings;
    type Error = PublishError;
    const ID: &'static str = if N == 0 {
        "custom.first"
    } else {
        "custom.second"
    };
    const INTERVAL: Duration = Duration::from_millis(5);
    const STREAMS: &'static [publications::Entry] = &[publications::Entry::of::<Published>()];

    async fn initialize(resources: &Settings) -> Result<Self, PublishError> {
        resources.state.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(Self(resources.state.clone()))
    }

    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Settings, Host>,
    ) -> Result<(), PublishError> {
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
    let resources = Arc::new(Settings {
        panic_first,
        ..Settings::default()
    });
    let state = resources.state.clone();
    let seed = Seed::new(&resources);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Bridge>::new(capacity).unwrap();
    assert_eq!(state.opened.load(Ordering::SeqCst), 0);
    assert!(SEEDS.lock().unwrap().contains_key(&seed.id));
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    assert!(!SEEDS.lock().unwrap().contains_key(&seed.id));
    tokio::time::timeout(
        Duration::from_secs(2),
        fixture.ready().wait_for(|ready| *ready),
    )
    .await
    .unwrap()
    .unwrap();
    let stream = fixture
        .runtime
        .streams
        .publication("custom-host.pulse", 1)
        .unwrap();
    let mut rows = stream.watch();
    let values = tokio::time::timeout(Duration::from_secs(2), async {
        let mut values = BTreeSet::new();
        while values.len() < if panic_first { 1 } else { 2 } {
            let item =
                std::future::poll_fn(|cx| futures_core::Stream::poll_next(Pin::new(&mut rows), cx))
                    .await;
            match item {
                Some(harmos::streams::Item::Row(row)) => {
                    values.insert(Pulse::decode(&row.value.payload).unwrap().sample);
                }
                Some(_) => {}
                None => panic!("custom host publication closed before producing"),
            }
        }
        values
    })
    .await
    .unwrap();
    assert!(values.iter().all(|value| [41, 42].contains(value)));
    assert_eq!(state.opened.load(Ordering::SeqCst), 1);
    assert_eq!(
        state.resource_address.load(Ordering::SeqCst),
        Arc::as_ptr(&resources) as usize
    );
    assert_eq!(state.bindings.load(Ordering::SeqCst), 2);
    assert_eq!(
        state.initialized.load(Ordering::SeqCst),
        if panic_first { 1 } else { 2 }
    );
    assert!(*fixture.ready().borrow());
    let owner = state.owner.lock().unwrap().unwrap();
    assert!(
        state
            .bound_on
            .lock()
            .unwrap()
            .iter()
            .all(|thread| *thread != owner)
    );
    let managed_stop = fixture.shutdown().await;
    let runtime_stop = fixture.runtime.stop().await;
    let stopped = managed_stop.and(runtime_stop);
    if panic_first {
        let error = stopped.expect_err("observer binding failure reaches root shutdown");
        let mut cause: &(dyn std::error::Error + 'static) = &error;
        loop {
            if cause
                .to_string()
                .contains("observer host or context factory panicked")
            {
                break;
            }
            if let Some(harmos::Error::TaskFailed { source, .. }) =
                cause.downcast_ref::<harmos::Error>()
            {
                cause = source.as_ref();
            } else if let Some(harmos::Error::PluginLifecycle { source }) =
                cause.downcast_ref::<harmos::Error>()
            {
                cause = source.as_ref();
            } else if let Some(harmos::plugin::PluginFailure::Cleanup(failures)) =
                cause.downcast_ref::<harmos::plugin::PluginFailure>()
            {
                assert_eq!(failures.len(), 1);
                cause = &failures[0].error;
            } else {
                cause = cause
                    .source()
                    .expect("structured binding failure is retained");
            }
        }
    } else {
        stopped.unwrap();
    }
    assert!(state.cleaned.load(Ordering::SeqCst));
    assert_eq!(
        state.initialized.load(Ordering::SeqCst),
        state.stopped.load(Ordering::SeqCst)
    );
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("custom_hosts_bind", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    exercise(false).await;
                });
        });
    }
    {
        scenario_runtime::check("binding_panic_isolated", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    exercise(true).await;
                });
        });
    }
}

/* === Tests === */
