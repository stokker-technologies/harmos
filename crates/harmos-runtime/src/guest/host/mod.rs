/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod envelope;
pub(crate) mod plugin;

mod sdk;
mod serving;
pub(crate) mod session;

/* === Re-Exports === */

pub use serving::Serving;

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use harmos_operations::Refusal as OperationRefusal;
use sha2::{Digest, Sha256};
use wasmtime::component::{Component, InstancePre, Linker};
use wasmtime::{Config, Engine as WasmEngine, Store, StoreLimits, StoreLimitsBuilder};

// Local Imports
use super::Declaration;
use crate::config::PluginGuestSettings;
use crate::{Application, Runtime, streams::StreamManager};
use harmos_protocol::v1 as host;
use prost::Message as _;

/* === Constants === */

const TICK: Duration = Duration::from_millis(1);

const UNBOUNDED: u64 = u64::MAX / 2;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Epochs {
    engine: WasmEngine,
}

pub struct Host<A: Application> {
    engine: WasmEngine,
    epochs: Arc<Epochs>,
    streams: StreamManager<A>,
    operations: crate::operations::Manager,
    max_component: usize,
    stream_window: Option<std::num::NonZeroUsize>,
    limits: StoreLimits,
    fuel: u64,
}

pub struct Loaded<A: Application> {
    sha256: [u8; 32],
    declaration: Declaration,
    component: InstancePre<Bridge<A>>,
    sdk: sdk::PluginIndices,
    streams: StreamManager<A>,
    operations: crate::operations::Manager,
    epochs: Arc<Epochs>,
    fuel: u64,
    limits: StoreLimits,
}

pub struct Registered<A: Application> {
    store: Store<Bridge<A>>,
    sdk: sdk::Plugin,
    routing: Option<crate::guest::host::plugin::PluginEndpoint>,
    epochs: Arc<Epochs>,
    fuel: u64,
}

struct Active<A: Application> {
    store: Store<Bridge<A>>,
    sdk: sdk::Plugin,
    routing: Option<crate::guest::host::plugin::PluginEndpoint>,
    // Keeps Epoch Ticking Alive Through Physical Guest Settlement
    _epochs: Arc<Epochs>,
    fuel: u64,
}

struct Bridge<A: Application> {
    declaration: Declaration,
    streams: StreamManager<A>,
    operations: crate::operations::Manager,
    publications: BTreeMap<String, Arc<crate::streams::StreamWriter>>,
    registration: Option<crate::streams::StreamRegistration>,
    scope: Option<crate::guest::host::plugin::PluginScope>,
    caller: Option<crate::operations::Caller>,
    children: Arc<Mutex<BTreeMap<[u8; 32], crate::operations::children::Children>>>,
    session: Option<std::sync::Weak<session::Endpoint>>,
    active: bool,
    limits: StoreLimits,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Routing(#[from] crate::guest::host::plugin::DispatchError),

    #[error(transparent)]
    OperationCaller(#[from] crate::Error),

    #[error(transparent)]
    Stream(#[from] crate::streams::StreamRegistrationError),

    #[error("guest component exceeds the {limit}-byte host bound")]
    Oversized { limit: usize },

    #[error("the Wasmtime guest engine could not be configured: {message}")]
    Engine { message: String },

    #[error("guest `{id}` is not a compatible component: {message}")]
    Component { id: String, message: String },

    #[error("the guest refused activation: {message}")]
    Refused { message: String },

    #[error("the guest host call was denied ({code}): {message}")]
    Denied { code: &'static str, message: String },

    #[error("the guest refused an operation: {refusal}")]
    Operation { refusal: OperationRefusal },

    #[error("the guest cannot answer `{method}`: {discarded}")]
    Unavailable { method: String, discarded: String },

    #[error("the guest answered `{method}` outside its route contract: {message}")]
    Malformed { method: String, message: String },

    #[error("the guest failed: {message}")]
    Guest { message: String },

    #[error("guest operation execution failed: {source}")]
    Execution {
        #[source]
        source: std::sync::Arc<wasmtime::Error>,
    },
}

/* === Traits === */

/* === Implementations === */

impl Epochs {
    fn start(engine: &WasmEngine, clock: crate::clock::Clock) -> Arc<Self> {
        let epochs = Arc::new(Self { engine: engine.clone() });
        let ticking = Arc::downgrade(&epochs);
        std::thread::spawn(move || {
            let executor =
                tokio::runtime::Builder::new_current_thread().enable_time().build().expect("the guest epoch clock executor builds");
            while let Some(epochs) = ticking.upgrade() {
                epochs.engine.increment_epoch();
                drop(epochs);
                executor.block_on(async { clock.sleep(TICK).await });
            }
        });

        epochs
    }
}

impl<A: Application> Host<A> {
    pub fn new(runtime: &Runtime<A>) -> Result<Self, Error> {
        Self::with_settings(runtime, PluginGuestSettings::default())
    }

    pub fn with_settings(runtime: &Runtime<A>, settings: PluginGuestSettings) -> Result<Self, Error> {
        Self::from_parts(runtime.streams.clone(), runtime.operations.execution(), settings)
    }

    pub(crate) fn from_parts(
        streams: StreamManager<A>, operations: crate::operations::Manager, settings: PluginGuestSettings,
    ) -> Result<Self, Error> {
        let mut config = Config::new();
        // Fuel bounds computation and the epoch bounds time, and neither
        // answers the other's question: a guest suspended inside a host
        // call burns no fuel and would run past any wall-clock budget
        // untouched, while a guest looping without host calls is stopped by
        // fuel whether or not a clock is watching.
        config
            .wasm_component_model(true)
            .wasm_component_model_async(true)
            .concurrency_support(true)
            .consume_fuel(true)
            .epoch_interruption(true);
        let engine = WasmEngine::new(&config).map_err(|source| Error::Engine { message: source.to_string() })?;
        let epochs = Epochs::start(&engine, operations.clock().clone());
        Ok(Self {
            engine,
            epochs,
            streams,
            operations,
            max_component: settings.component_bytes.get(),
            stream_window: settings.stream_window,
            fuel: settings.fuel.get(),
            limits: store_limits(settings),
        })
    }

    pub fn load(&self, bytes: &[u8]) -> Result<Loaded<A>, Error> {
        if bytes.len() > self.max_component {
            return Err(Error::Oversized { limit: self.max_component });
        }

        let registration = crate::bundle::guest_registration(bytes).map_err(|error| Error::Guest { message: error.to_string() })?;
        let declaration = Declaration::new(registration, self.stream_window)?;
        let component = Component::new(&self.engine, bytes).map_err(|source| Error::Component {
            id: "guest".into(),
            message: source.to_string(),
        })?;
        let mut linker: Linker<Bridge<A>> = Linker::new(&self.engine);
        linker
            .instance("harmos:guest/operation-context@1.0.0")
            .and_then(|mut interface| {
                interface.func_wrap_concurrent("child", |accessor, (payload,): (Vec<u8>,)| {
                    Box::pin(async move {
                        let submitted = crate::supervision::executor::now();
                        let child = crate::operations::WireChild::decode(&payload).and_then(|mut child| {
                            let preparation = if let crate::operations::WireChild::Call(call) = &mut child {
                                accessor.with(|mut access| access.get().publication_payload(call))?
                            } else {
                                None
                            };
                            Ok((child, preparation))
                        });
                        let result = match child {
                            Err(error) => Err(Bridge::<A>::refusal("-32602", error.to_string())),
                            Ok((child, _preparation)) => {
                                let scope = accessor.with(|mut access| {
                                    access.get().children.lock().expect("guest child context lock").get(child.context()).cloned()
                                });
                                match (scope, child) {
                                    (None, crate::operations::WireChild::Control(_)) => {
                                        Err(Bridge::<A>::refusal("-32601", "submitted children require an attempt context"))
                                    }
                                    (None, crate::operations::WireChild::Call(call)) if call.submit => {
                                        Err(Bridge::<A>::refusal("-32601", "submitted children require an attempt context"))
                                    }
                                    (None, crate::operations::WireChild::Call(call)) => {
                                        let endpoint =
                                            accessor.with(|mut access| access.get().session.as_ref().and_then(std::sync::Weak::upgrade));
                                        match endpoint {
                                            Some(endpoint) => endpoint.background(*call, submitted).await.map_err(|error| {
                                                let refusal = match error {
                                                    Error::OperationCaller(error) => crate::operations::children::refused(error),
                                                    error => harmos_operations::Refusal::new(-32_606, error),
                                                };
                                                Bridge::<A>::forwarded(&refusal)
                                            }),
                                            None => Err(Bridge::<A>::refusal("-32601", "guest calling context is unavailable")),
                                        }
                                    }
                                    (Some(scope), child) => {
                                        let (_cancel, cancelled) = tokio::sync::oneshot::channel();
                                        let answer = match child {
                                            crate::operations::WireChild::Call(call) => scope.call(*call, submitted, cancelled).await,
                                            crate::operations::WireChild::Control(control) => scope.control(control, cancelled).await,
                                        };
                                        answer.map_err(|error| Bridge::<A>::forwarded(&error))
                                    }
                                }
                            }
                        };
                        Ok((result,))
                    })
                })
            })
            .map_err(|source| Error::Component {
                id: declaration.id.clone(),
                message: source.to_string(),
            })?;
        let component = linker.instantiate_pre(&component).map_err(|source| Error::Component {
            id: declaration.id.clone(),
            message: source.to_string(),
        })?;
        let sdk = sdk::PluginIndices::new(&component).map_err(|source| Error::Component {
            id: declaration.id.clone(),
            message: source.to_string(),
        })?;
        Ok(Loaded {
            sha256: Sha256::digest(bytes).into(),
            declaration,
            component,
            sdk,
            streams: self.streams.clone(),
            operations: self.operations.clone(),
            epochs: Arc::clone(&self.epochs),
            fuel: self.fuel,
            limits: self.limits.clone(),
        })
    }
}

impl<A: Application> Loaded<A> {
    pub(crate) fn validate_publications(&self) -> Result<(), Error> {
        self.streams.validate_publications(&self.declaration.id, self.declaration.streams.clone())?;
        Ok(())
    }

    pub(crate) fn duplicate(&self) -> Result<Self, Error> {
        let component = self.component.clone();
        let sdk = sdk::PluginIndices::new(&component).map_err(|source| Error::Component {
            id: self.declaration.id.clone(),
            message: source.to_string(),
        })?;
        Ok(Self {
            sha256: self.sha256,
            declaration: self.declaration.clone(),
            component,
            sdk,
            streams: self.streams.clone(),
            operations: self.operations.clone(),
            epochs: self.epochs.clone(),
            fuel: self.fuel,
            limits: self.limits.clone(),
        })
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    pub async fn register(self) -> Result<Registered<A>, Error> {
        let fuel = self.fuel;
        let bridge = Bridge {
            declaration: self.declaration,
            streams: self.streams,
            operations: self.operations,
            publications: BTreeMap::new(),
            registration: None,
            scope: None,
            caller: None,
            children: Arc::new(Mutex::new(BTreeMap::new())),
            session: None,
            active: false,
            limits: self.limits,
        };
        let mut store = Store::new(self.component.engine(), bridge);
        store.limiter(|bridge| &mut bridge.limits);
        store.set_fuel(fuel).map_err(|source| Error::Guest { message: format!("{source:#}") })?;
        store.set_epoch_deadline(UNBOUNDED);
        let instance =
            self.component.instantiate_async(&mut store).await.map_err(|source| Error::Guest { message: format!("{source:#}") })?;
        let sdk = self.sdk.load(&mut store, &instance).map_err(|source| Error::Guest { message: format!("{source:#}") })?;
        // Collect Inert Metadata on the Original Store Before P Exists
        let collected = store.run_concurrent(async |accessor| sdk.call_register(accessor).await).await.and_then(|result| result);
        let metadata = match collected {
            Ok(Ok(bytes)) if bytes.len() <= harmos_protocol::guest::COMMAND_BYTES => {
                v1::GuestRegistration::decode(bytes.as_slice()).map_err(|error| Error::Guest { message: error.to_string() })?
            }
            Ok(Ok(_)) => {
                return Err(Error::Guest {
                    message: "Guest Registration Exceeds Its Byte Bound".into(),
                });
            }
            Ok(Err(message)) => return Err(Error::Refused { message }),
            Err(source) => return Err(Error::Guest { message: format!("{source:#}") }),
        };
        if metadata != store.data().declaration.registration {
            return Err(Error::Guest {
                message: "Guest Registration Changed Between Stores".into(),
            });
        }
        Ok(Registered {
            store,
            sdk,
            routing: None,
            epochs: self.epochs,
            fuel: self.fuel,
        })
    }
}

impl<A: Application> Registered<A> {
    pub fn routed(mut self, router: &crate::guest::host::plugin::Router) -> Result<Self, Error> {
        let instance = router.enter(&self.store.data().declaration.id)?;
        self.store.data_mut().scope = instance.scope().cloned();
        let caller = self.store.data().operations.caller(&instance.caller())?;
        self.store.data_mut().caller = Some(caller);
        self.routing = Some(instance);
        Ok(self)
    }

    pub async fn serve_sdk(mut self) -> Result<Serving<A>, Error> {
        let scope = self.store.data().scope.as_ref().ok_or_else(|| Error::Unavailable {
            method: "serve_sdk".to_owned(),
            discarded: "guest has no original managed scope".to_owned(),
        })?;
        let manifest = &self.store.data().declaration.contracts;
        if scope.lifecycle() != Some(&manifest.lifecycle) {
            return Err(Error::Guest {
                message: "guest SDK pair differs from the original frozen generation".to_owned(),
            });
        }
        scope.validate_operations(self.store.data().declaration.operations.iter())?;
        scope.validate_wire_contracts(manifest.provides.iter())?;
        let mut required = manifest.requires.clone();
        required.sort_by(|a, b| a.contract.operation().id.cmp(&b.contract.operation().id));
        if scope.required_contracts().is_none_or(|actual| {
            actual.len() != required.len() || actual.iter().zip(&required).any(|(actual, expected)| actual.contract != expected.contract)
        }) {
            return Err(crate::Error::InvalidInput {
                field: "guest required contracts",
                reason: "manifest differs from the original admitted generation",
            }
            .into());
        }
        self.store.data_mut().activate().await?;
        Active {
            store: self.store,
            sdk: self.sdk,
            routing: self.routing,
            _epochs: self.epochs,
            fuel: self.fuel,
        }
        .serve()
    }
}

impl<A: Application> Bridge<A> {
    async fn activate(&mut self) -> Result<(), Error> {
        if self.active {
            return Ok(());
        }
        let scope = self.scope.as_ref().ok_or_else(|| Error::Unavailable {
            method: "activate".to_owned(),
            discarded: "guest has no original managed scope".to_owned(),
        })?;
        let registration = self.streams.stage_plugin(scope, self.declaration.streams.clone()).await?;
        let leases = registration.leases().to_vec();
        self.registration = Some(registration);
        self.publications =
            self.declaration.streams.iter().zip(leases).map(|(declaration, lease)| (declaration.id.clone(), lease)).collect();
        self.active = true;
        Ok(())
    }

    fn publication_payload(
        &self, call: &mut crate::operations::WireChildCall,
    ) -> Result<Option<tokio::sync::OwnedSemaphorePermit>, crate::operations::WireCodecError> {
        if call.operation != "harmos.samples.append" && !call.operation.ends_with(".harmos.samples.append") {
            return Ok(None);
        }
        let factory = harmos_operations::execution::operations::OperationFactory::protobuf::<harmos_operations::streams::Append>()
            .map_err(|_| crate::operations::WireCodecError::Shape)?;
        let bytes = factory.grpc_decoding_memory(&call.payload).map_err(|_| crate::operations::WireCodecError::Shape)?;
        let preparation = self.operations.claim_preparation(std::num::NonZeroUsize::new(bytes.max(1)).unwrap())?;
        let mut request = harmos_protocol::context::SamplesAppendRequest::decode(call.payload.as_slice())?;
        let catalog = self.declaration.registration.catalog.as_ref().ok_or(crate::operations::WireCodecError::Shape)?;
        let stream = catalog.streams.iter().find(|stream| stream.id == request.stream).ok_or(crate::operations::WireCodecError::Shape)?;
        let pool =
            prost_reflect::DescriptorPool::from_file_descriptor_set(catalog.types.clone().ok_or(crate::operations::WireCodecError::Shape)?)
                .map_err(|_| crate::operations::WireCodecError::Shape)?;
        let row =
            crate::streams::protobuf::Row::new(pool.get_message_by_name(&stream.row).ok_or(crate::operations::WireCodecError::Shape)?)
                .map_err(|_| crate::operations::WireCodecError::Shape)?;
        request.payload = row.decode(&request.payload).map_err(|_| crate::operations::WireCodecError::Shape)?;
        for following in &mut request.following {
            following.payload = row.decode(&following.payload).map_err(|_| crate::operations::WireCodecError::Shape)?;
        }
        call.payload = request.encode_to_vec();
        Ok(Some(preparation))
    }

    fn refusal(code: &str, message: impl Into<String>) -> Vec<u8> {
        v1::Failure {
            classification: v1::Classification::Refused as i32,
            code: code.into(),
            message: message.into(),
            ..Default::default()
        }
        .encode_to_vec()
    }
    fn forwarded(refusal: &OperationRefusal) -> Vec<u8> {
        let mut failure = v1::Failure::from(refusal.failure());
        if failure.code.is_empty() {
            failure.code = refusal.code().to_string();
        }
        failure.encode_to_vec()
    }
}

/* === Public Functions === */

/// Collect the Authoritative Guest Metadata Without Granting Host Authority.
pub async fn inspect(bytes: &[u8], settings: PluginGuestSettings) -> Result<v1::GuestRegistration, Error> {
    if bytes.len() > settings.component_bytes.get() {
        return Err(Error::Oversized { limit: settings.component_bytes.get() });
    }
    let mut config = Config::new();
    config.wasm_component_model(true).wasm_component_model_async(true).concurrency_support(true).consume_fuel(true);
    let engine = WasmEngine::new(&config).map_err(|error| Error::Guest { message: error.to_string() })?;
    let component = Component::new(&engine, bytes).map_err(|error| Error::Guest { message: error.to_string() })?;
    inspect_component(&component, store_limits(settings), settings.fuel.get()).await
}

/* === Private Functions === */

async fn inspect_component(component: &Component, limits: StoreLimits, fuel: u64) -> Result<v1::GuestRegistration, Error> {
    let mut linker: Linker<StoreLimits> = Linker::new(component.engine());
    linker
        .instance("harmos:guest/operation-context@1.0.0")
        .and_then(|mut interface| {
            interface.func_wrap_concurrent("child", |_, (_payload,): (Vec<u8>,)| {
                Box::pin(async {
                    Ok((Err::<Vec<u8>, _>(
                        v1::Failure {
                            classification: v1::Classification::Refused as i32,
                            code: "registration_has_no_authority".into(),
                            message: "Registration Cannot Call the Host".into(),
                            ..Default::default()
                        }
                        .encode_to_vec(),
                    ),))
                })
            })
        })
        .map_err(|error| Error::Guest { message: error.to_string() })?;
    let pre = linker.instantiate_pre(component).map_err(|error| Error::Guest { message: error.to_string() })?;
    let indices = sdk::PluginIndices::new(&pre).map_err(|error| Error::Guest { message: error.to_string() })?;
    let mut store = Store::new(component.engine(), limits);
    store.limiter(|limits| limits);
    store.set_fuel(fuel).map_err(|error| Error::Guest { message: error.to_string() })?;
    store.set_epoch_deadline(UNBOUNDED);
    let instance = pre.instantiate_async(&mut store).await.map_err(|error| Error::Guest { message: format!("{error:#}") })?;
    let sdk = indices.load(&mut store, &instance).map_err(|error| Error::Guest { message: error.to_string() })?;
    let bytes = store
        .run_concurrent(async |accessor| sdk.call_register(accessor).await)
        .await
        .and_then(|result| result)
        .map_err(|error| Error::Guest { message: format!("{error:#}") })?
        .map_err(|message| Error::Refused { message })?;
    if bytes.len() > harmos_protocol::guest::COMMAND_BYTES {
        return Err(Error::Guest {
            message: "Guest Registration Exceeds Its Byte Bound".into(),
        });
    }
    v1::GuestRegistration::decode(bytes.as_slice()).map_err(|error| Error::Guest { message: error.to_string() })
}

pub(super) fn store_limits(settings: PluginGuestSettings) -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(settings.memory_bytes.get())
        .memories(settings.memories.get())
        .table_elements(settings.table_elements.get())
        .tables(settings.tables.get())
        .instances(settings.instances.get())
        .trap_on_grow_failure(true)
        .build()
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{Clock, Reading, Sleep, Source};
    use tokio::sync::{mpsc, oneshot};
    use tokio::time::Instant;

    struct ControlledClock {
        instant: Instant,
        waits: mpsc::UnboundedSender<(Instant, oneshot::Sender<()>)>,
    }

    impl Source for ControlledClock {
        fn read(&self) -> Reading {
            Reading {
                instant: self.instant,
                date: std::time::SystemTime::UNIX_EPOCH,
            }
        }

        fn sleep_until(&self, deadline: Instant) -> Sleep {
            let (wake, waiting) = oneshot::channel();
            self.waits.send((deadline, wake)).unwrap();
            Box::pin(async move { waiting.await.unwrap() })
        }
    }

    #[tokio::test]
    async fn epoch_thread_waits_on_the_host_clock_and_releases_its_owner() {
        // Start Epoch Interruption With A Controlled Host Source
        let instant = Instant::now();
        let (waits, mut observed) = mpsc::unbounded_channel();
        let clock = Clock::with_clock(ControlledClock { instant, waits });
        let epochs = Epochs::start(&WasmEngine::default(), clock);

        // The Independent Thread Registers Its Wait On That Source
        let (deadline, wake) = observed.recv().await.unwrap();
        assert!(deadline > instant);

        // Releasing The Guest Owner Ends The Thread After Its Pending Wait
        drop(epochs);
        wake.send(()).unwrap();
        assert!(observed.recv().await.is_none());
    }
}
