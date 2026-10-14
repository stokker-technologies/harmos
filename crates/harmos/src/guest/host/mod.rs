/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
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
use harmos_contract::Refusal as OperationRefusal;
use sha2::{Digest, Sha256};
use wasmtime::component::{Component, InstancePre, Linker};
use wasmtime::{Config, Engine as WasmEngine, Store, StoreLimits, StoreLimitsBuilder};

// Local Imports
use super::Registration;
use crate::config::PluginGuestSettings;
use crate::{Application, Runtime, streams::StreamManager};
use harmos_contract::inventory::{Declaration, DeclarationError, Facility};

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
    operations: crate::operations::OperationManager,
    max_component: usize,
    limits: StoreLimits,
    fuel: u64,
}

pub struct Loaded<A: Application> {
    sha256: [u8; 32],
    declaration: Declaration,
    component: InstancePre<Bridge<A>>,
    sdk: sdk::PluginIndices,
    streams: StreamManager<A>,
    operations: crate::operations::OperationManager,
    epochs: Arc<Epochs>,
    fuel: u64,
    limits: StoreLimits,
}

pub struct Registered<A: Application> {
    store: Store<Bridge<A>>,
    sdk: sdk::Plugin,
    registrations: Vec<Registration>,
    routing: Option<crate::plugin::PluginEndpoint>,
    epochs: Arc<Epochs>,
    fuel: u64,
}

struct Active<A: Application> {
    store: Store<Bridge<A>>,
    sdk: sdk::Plugin,
    routing: Option<crate::plugin::PluginEndpoint>,
    // Keeps Epoch Ticking Alive Through Physical Guest Settlement
    _epochs: Arc<Epochs>,
    fuel: u64,
}

struct Bridge<A: Application> {
    declaration: Declaration,
    streams: StreamManager<A>,
    operations: crate::operations::OperationManager,
    publications: BTreeMap<String, Arc<crate::streams::PublicationLease>>,
    registration: Option<crate::streams::StreamRegistration>,
    scope: Option<crate::plugin::PluginScope>,
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
    Routing(#[from] crate::plugin::DispatchError),
    #[error(transparent)]
    OperationCaller(#[from] crate::Error),
    #[error(transparent)]
    Stream(#[from] crate::streams::StreamAdmissionError),
    #[error(transparent)]
    Declaration(#[from] DeclarationError),
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
    fn start(engine: &WasmEngine) -> Arc<Self> {
        let epochs = Arc::new(Self {
            engine: engine.clone(),
        });
        let ticking = Arc::downgrade(&epochs);
        std::thread::spawn(move || {
            while let Some(clock) = ticking.upgrade() {
                clock.engine.increment_epoch();
                drop(clock);
                std::thread::sleep(TICK);
            }
        });

        epochs
    }

    fn ticks(&self, bound: Duration) -> u64 {
        u64::try_from(bound.as_nanos().div_ceil(TICK.as_nanos()))
            .unwrap_or(UNBOUNDED)
            .clamp(1, UNBOUNDED)
    }
}

impl<A: Application> Host<A> {
    pub fn new(runtime: &Runtime<A>) -> Result<Self, Error> {
        Self::with_settings(runtime, PluginGuestSettings::default())
    }

    pub fn with_settings(
        runtime: &Runtime<A>,
        settings: PluginGuestSettings,
    ) -> Result<Self, Error> {
        Self::from_parts(
            runtime.streams.clone(),
            runtime.operations.clone(),
            settings,
        )
    }

    pub(crate) fn from_parts(
        streams: StreamManager<A>,
        operations: crate::operations::OperationManager,
        settings: PluginGuestSettings,
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
        let engine = WasmEngine::new(&config).map_err(|source| Error::Engine {
            message: source.to_string(),
        })?;
        let epochs = Epochs::start(&engine);
        Ok(Self {
            engine,
            epochs,
            streams,
            operations,
            max_component: settings.component_bytes.get(),
            fuel: settings.fuel.get(),
            limits: store_limits(settings),
        })
    }

    pub fn load(&self, bytes: &[u8]) -> Result<Loaded<A>, Error> {
        if bytes.len() > self.max_component {
            return Err(Error::Oversized {
                limit: self.max_component,
            });
        }
        let declaration = Declaration::inspect(bytes, Facility::Guest)?;
        let component = Component::new(&self.engine, bytes).map_err(|source| Error::Component {
            id: declaration.id.clone(),
            message: source.to_string(),
        })?;
        let mut linker: Linker<Bridge<A>> = Linker::new(&self.engine);
        linker
            .instance("harmos:guest/operation-context@0.2.0")
            .and_then(|mut interface| {
                interface.func_wrap_concurrent("child", |accessor, (payload,): (Vec<u8>,)| {
                    Box::pin(async move {
                        let submitted = crate::supervision::executor::now();
                        let child = crate::operations::WireChild::decode(&payload);
                        let result = match child {
                            Err(error) => Err(Bridge::<A>::refusal("-32602", error.to_string())),
                            Ok(child) => {
                                let scope = accessor.with(|mut access| {
                                    access
                                        .get()
                                        .children
                                        .lock()
                                        .expect("guest child context lock")
                                        .get(child.context())
                                        .cloned()
                                });
                                match (scope, child) {
                                    (None, crate::operations::WireChild::Control(_)) => {
                                        Err(Bridge::<A>::refusal(
                                            "-32601",
                                            "submitted children require an attempt context",
                                        ))
                                    }
                                    (None, crate::operations::WireChild::Call(call))
                                        if call.submit =>
                                    {
                                        Err(Bridge::<A>::refusal(
                                            "-32601",
                                            "submitted children require an attempt context",
                                        ))
                                    }
                                    (None, crate::operations::WireChild::Call(call)) => {
                                        let endpoint = accessor.with(|mut access| {
                                            access
                                                .get()
                                                .session
                                                .as_ref()
                                                .and_then(std::sync::Weak::upgrade)
                                        });
                                        match endpoint {
                                            Some(endpoint) => endpoint
                                                .background(call, submitted)
                                                .await
                                                .map_err(|error| {
                                                    let refusal = match error {
                                                        Error::OperationCaller(error) => {
                                                            crate::operations::children::refused(
                                                                error,
                                                            )
                                                        }
                                                        error => harmos_contract::Refusal::new(
                                                            -32_606, error,
                                                        ),
                                                    };
                                                    Bridge::<A>::refusal(
                                                        &refusal.code().to_string(),
                                                        refusal.message(),
                                                    )
                                                }),
                                            None => Err(Bridge::<A>::refusal(
                                                "-32601",
                                                "guest calling context is unavailable",
                                            )),
                                        }
                                    }
                                    (Some(scope), child) => {
                                        let (_cancel, cancelled) = tokio::sync::oneshot::channel();
                                        let answer = match child {
                                            crate::operations::WireChild::Call(call) => {
                                                scope.call(call, submitted, cancelled).await
                                            }
                                            crate::operations::WireChild::Control(control) => {
                                                scope.control(control, cancelled).await
                                            }
                                        };
                                        answer.map_err(|error| {
                                            Bridge::<A>::refusal(
                                                &error.code().to_string(),
                                                error.message(),
                                            )
                                        })
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
        let component = linker
            .instantiate_pre(&component)
            .map_err(|source| Error::Component {
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
        self.register_with_limits(None).await
    }

    pub(super) async fn register_with_limits(
        self,
        limits: Option<StoreLimits>,
    ) -> Result<Registered<A>, Error> {
        // Catalog serialization can legitimately do more work than one
        // interactive route call; its wall-clock bound remains separate.
        let fuel = if limits.is_some() {
            100_000_000
        } else {
            self.fuel
        };
        let deadline = if limits.is_some() {
            self.epochs.ticks(Duration::from_secs(10))
        } else {
            UNBOUNDED
        };
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
            limits: limits.unwrap_or(self.limits),
        };
        let mut store = Store::new(self.component.engine(), bridge);
        store.limiter(|bridge| &mut bridge.limits);
        store.set_fuel(fuel).map_err(|source| Error::Guest {
            message: format!("{source:#}"),
        })?;
        store.set_epoch_deadline(deadline);
        let instance = self
            .component
            .instantiate_async(&mut store)
            .await
            .map_err(|source| Error::Guest {
                message: format!("{source:#}"),
            })?;
        let sdk = self
            .sdk
            .load(&mut store, &instance)
            .map_err(|source| Error::Guest {
                message: format!("{source:#}"),
            })?;
        // Collect Inert Metadata On The Original Store Before P Exists
        let collected = store
            .run_concurrent(async |accessor| sdk.call_register(accessor).await)
            .await
            .and_then(|result| result);
        let registrations = match collected {
            Ok(Ok(registrations)) => Ok(registrations
                .into_iter()
                .map(|entry| Registration {
                    namespace: entry.namespace,
                    schema: entry.schema,
                    payload: entry.payload,
                })
                .collect()),
            Ok(Err(message)) => Err(Error::Refused { message }),
            Err(source) => Err(Error::Guest {
                message: format!("{source:#}"),
            }),
        }?;
        Ok(Registered {
            store,
            sdk,
            registrations,
            routing: None,
            epochs: self.epochs,
            fuel: self.fuel,
        })
    }
}

impl<A: Application> Registered<A> {
    pub fn registrations(&self) -> &[Registration] {
        &self.registrations
    }

    pub fn routed(mut self, router: &crate::plugin::Router) -> Result<Self, Error> {
        let instance = router.enter(&self.store.data().declaration.id)?;
        self.store.data_mut().scope = instance.scope().cloned();
        let caller = self.store.data().operations.caller(&instance.caller())?;
        self.store.data_mut().caller = Some(caller);
        self.routing = Some(instance);
        Ok(self)
    }

    pub async fn serve_sdk(mut self) -> Result<Serving<A>, Error> {
        let scope = self
            .store
            .data()
            .scope
            .as_ref()
            .ok_or_else(|| Error::Unavailable {
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
        if scope.required_contracts() != Some(required.as_slice()) {
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
        let registration = self
            .streams
            .stage_plugin(scope, self.declaration.streams.clone())
            .await?;
        let leases = registration.leases().to_vec();
        self.registration = Some(registration);
        self.publications = self
            .declaration
            .streams
            .iter()
            .zip(leases)
            .map(|(declaration, lease)| (declaration.id.clone(), lease))
            .collect();
        self.active = true;
        Ok(())
    }

    fn refusal(code: &str, message: impl Into<String>) -> sdk::capabilities::Refusal {
        sdk::capabilities::Refusal {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
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

fn refused(refusal: sdk::capabilities::Refusal) -> OperationRefusal {
    match refusal.code.parse::<i64>() {
        Ok(code) => OperationRefusal::new(code, refusal.message),
        Err(_) => OperationRefusal::of(format!("{}: {}", refusal.code, refusal.message)),
    }
}

/* === Tests === */
