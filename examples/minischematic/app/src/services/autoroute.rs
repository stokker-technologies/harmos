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
pub use minischematic_contract::autoroute::Progress;

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::app::{Runtime, SchematicEditor};
use crate::transactions::net::RenameNet;
use harmos::guest::{Host, Loaded, Registered, Serving};
use harmos::load::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions,
    PluginLifecycle, PluginLifecycleAuthority, PluginManager, PluginTransition, Router,
};
use harmos::operations::{
    Context, EncodedInvocation, OperationDeclaration, PluginRegistration, PluginSdkRegistration,
    host,
};
use harmos::{Identified, PrincipalId, PublicationRow, RequestKey, StateManager};
use minischematic_contract::autoroute::{NET_RENAME, Operations};

/* === Constants === */
pub const NAME: &str = "autoroute";

const SDK_ENCODING_MEMORY_BYTES: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/* === Macros === */

/* === Type Aliases === */
pub type Manager = PluginManager<Guest, (), ()>;
type Backend = PluginBackend<Guest, (), ()>;

/* === Structs === */
pub struct Provider {
    state: StateManager<SchematicEditor>,
}

struct Publish {
    _resources: Arc<Provider>,
}

struct Invoke {
    resources: Arc<Provider>,
}

pub struct Guest {
    registered: Option<Registered<SchematicEditor>>,
    serving: Option<Serving<SchematicEditor>>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
}

pub struct Lifecycle {
    operations: harmos::OperationManager,
    guest: Option<Guest>,
}

/* === Enums === */
#[harmos::lanes]
pub enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Analysis,
}

/* === Traits === */

/* === Implementations === */
impl PluginLifecycle for Lifecycle {
    type Guest = Guest;
    type Sidecar = ();
    type Embedded = ();

    fn preflight(&mut self, candidate: &PluginDefinitions) -> harmos::Result<()> {
        self.operations.validate_requirements(candidate)
    }

    fn prepare(&mut self, definition: &PluginDefinition) -> harmos::Result<Backend> {
        if definition.id != NAME || !matches!(definition.artifact, PluginArtifact::Guest { .. }) {
            return Err(harmos::Error::InvalidInput {
                field: "autoroute artifact",
                reason: "this lifecycle owns the inspected Autoroute guest",
            });
        }
        let guest = self.guest.take().ok_or(harmos::Error::InvalidInput {
            field: "autoroute generation",
            reason: "the inspected guest store was already transferred",
        })?;
        Ok(PluginBackend::Guest(guest))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> harmos::Result<()> {
        // The Walkthrough Has No Concurrent Application Executions To Drain.
        Ok(())
    }

    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut Backend,
    ) -> harmos::Result<()> {
        let PluginBackend::Guest(guest) = backend else {
            unreachable!("prepare admits only the inspected guest")
        };
        let registered = guest
            .registered
            .take()
            .expect("one original inspected store");
        let registered = registered.routed(router).map_err(guest_error)?;
        guest.serving = Some(registered.serve_sdk().await.map_err(guest_error)?);
        let serving = guest.serving.as_ref().unwrap();
        let component = Arc::new(serving.operations().map_err(guest_error)?);
        guest.registration = Some(self.operations.stage_plugin(
            serving.scope().unwrap(),
            |bindings| {
                bindings.register_component_catalog::<Operations>(Lanes::Open, component.clone())
            },
        )?);
        guest.sdk = Some(component.stage_sdk(&self.operations, Lanes::Open)?);
        guest.initializing = Some(guest.sdk.as_ref().unwrap().submit_initialize(
            authority,
            &definition.configuration,
            SDK_ENCODING_MEMORY_BYTES,
        )?);
        let result = guest.initializing.as_mut().unwrap().await;
        guest.initializing.take();
        result?;
        guest.initialized = true;
        Ok(())
    }

    async fn stop(
        _: &PluginDefinition,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut Backend,
    ) -> harmos::Result<()> {
        let PluginBackend::Guest(guest) = backend else {
            unreachable!("prepare admits only the inspected guest")
        };
        if let Some(sdk) = &guest.sdk {
            sdk.settle().await?;
            if let Some(initializing) = &mut guest.initializing {
                let result = initializing.await;
                guest.initializing.take();
                result?;
                guest.initialized = true;
            }
            if guest.initialized && !guest.shutdown_attempted {
                guest.shutting_down = Some(sdk.submit_shutdown(authority)?);
                guest.shutdown_attempted = true;
            }
            if let Some(shutdown) = &mut guest.shutting_down {
                let result = shutdown.await;
                guest.shutting_down.take();
                result?;
                guest.initialized = false;
            }
        }
        if let Some(registration) = &mut guest.registration {
            registration.shutdown().await?;
        }
        if let Some(serving) = &mut guest.serving {
            serving.finalize(Duration::from_secs(30)).await?;
        }
        if let Some(sdk) = &mut guest.sdk {
            sdk.close().await?;
        }
        guest.sdk.take();
        guest.registration.take();
        guest.serving.take();
        guest.registered.take();
        Ok(())
    }
}

#[harmos::operation(declaration = host::Publish)]
impl Publish {
    fn new(resources: Arc<Provider>) -> Self {
        Self {
            _resources: resources,
        }
    }

    async fn run(
        &mut self,
        context: Context<'_>,
        row: &mut host::Publication,
    ) -> Result<(), harmos::Error> {
        let scope = context.caller_scope().ok_or(harmos::Error::InvalidInput {
            field: "autoroute caller",
            reason: "publication needs its original guest generation",
        })?;
        if row.sequence.is_some() || row.published_unix_micros.is_some() || row.context.is_some() {
            return Err(harmos::Error::InvalidInput {
                field: "autoroute publication",
                reason: "guest publication retains host-derived sequence and original guest metadata",
            });
        }
        let lease = scope.publication(&row.stream, row.schema)?;
        let publication = PublicationRow {
            payload: std::mem::take(&mut row.payload),
            published_unix_micros: None,
            context: None,
        };
        lease.stage_derived_row(publication, 0).map_err(|source| {
            harmos::Error::OperationFailed {
                operation: host::Publish::ID.into(),
                source: Arc::new(source),
            }
        })?;
        Ok(())
    }
}

#[harmos::operation(declaration = host::Invoke)]
impl Invoke {
    fn new(resources: Arc<Provider>) -> Self {
        Self { resources }
    }

    async fn run(
        &mut self,
        context: Context<'_>,
        request: &mut host::InvokeRequest,
    ) -> Result<host::Receipt, harmos::Error> {
        let scope = context.caller_scope().ok_or(harmos::Error::InvalidInput {
            field: "autoroute caller",
            reason: "invocation needs its original guest generation",
        })?;
        if request.grant != NET_RENAME {
            return Err(harmos::Error::OperationUnauthorized {
                caller: scope.owner().into(),
                owner: NAME.into(),
                operation: request.grant.clone(),
            });
        }
        let journal = self
            .resources
            .state
            .as_principal(PrincipalId::new(format!("guest:{}", scope.owner())));
        let (id, version) = RenameNet::definition();
        let transaction = journal
            .decode_transaction(&id, version, &request.payload)
            .map_err(|source| harmos::Error::OperationFailed {
                operation: host::Invoke::ID.into(),
                source: Arc::new(source),
            })?;
        let receipt = journal
            .commit_keyed(
                RequestKey::new(std::mem::take(&mut request.key)),
                transaction,
            )
            .await
            .map_err(|source| harmos::Error::OperationFailed {
                operation: host::Invoke::ID.into(),
                source: Arc::new(source),
            })?;
        Ok(host::Receipt {
            position: receipt.position.0,
            correlation: receipt.correlation.0.0,
        })
    }
}

/* === Public Functions === */
pub async fn lifecycle(
    runtime: &Runtime,
    loaded: Loaded<SchematicEditor>,
) -> harmos::Result<(Manager, Lifecycle)> {
    let declaration = loaded.declaration().clone();
    if declaration.id != NAME {
        return Err(harmos::Error::InvalidInput {
            field: "autoroute artifact",
            reason: "the inspected component has a different owner",
        });
    }
    let contracts = &declaration.contracts;
    let sha256 = loaded.sha256();
    let registered = loaded.register().await.map_err(guest_error)?;
    let definition = PluginDefinition {
        id: declaration.id.clone(),
        version: declaration.version,
        artifact: PluginArtifact::Guest { sha256 },
        requires: Vec::new(),
        configuration: b"{}".to_vec(),
        contributions: PluginContributions {
            lifecycle: Some(contracts.lifecycle.clone()),
            operations: declaration.operations,
            wire_contracts: contracts.provides.clone(),
            required_contracts: Some(contracts.requires.clone()),
            sections: declaration.sections,
            streams: declaration.streams,
            listeners: declaration.listeners,
            registrations: registered.registrations().to_vec(),
            ..PluginContributions::default()
        },
    };
    let candidate = PluginDefinitions::new([definition], [])?;
    runtime.operations.validate_requirements(&candidate)?;
    let manager = Manager::new(candidate)?;
    let lifecycle = Lifecycle {
        operations: runtime.operations.clone(),
        guest: Some(Guest {
            registered: Some(registered),
            serving: None,
            registration: None,
            sdk: None,
            initializing: None,
            shutting_down: None,
            initialized: false,
            shutdown_attempted: false,
        }),
    };
    Ok((manager, lifecycle))
}

pub fn register_host(runtime: &Runtime) -> harmos::Result<()> {
    let owner = Arc::new(Provider {
        state: runtime.state.clone(),
    });
    runtime.operations.register_wire_operation::<Publish>(
        "autoroute-host",
        Lanes::Open,
        owner.clone(),
    )?;
    runtime
        .operations
        .register_wire_operation::<Invoke>("autoroute-host", Lanes::Open, owner)
}

pub fn host(runtime: &Runtime) -> Result<Host<SchematicEditor>, harmos::guest::Error> {
    Host::new(runtime)
}

pub fn load(
    runtime: &Runtime,
    component: &[u8],
) -> Result<Loaded<SchematicEditor>, harmos::guest::Error> {
    host(runtime)?.load(component)
}

/* === Private Functions === */
fn guest_error(source: harmos::guest::Error) -> harmos::Error {
    harmos::Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
