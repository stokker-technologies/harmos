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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

// Local Imports
use crate::app::{Runtime, SchematicEditor};
use crate::lanes::Lanes;
use harmos::operations::{EncodedInvocation, PluginRegistration, PluginSdkRegistration};
use harmos::plugin::guest::{Host, Loaded, Registered, Serving};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions,
    PluginLifecycle, PluginLifecycleAuthority, PluginManager, PluginTransition, Router,
};
use schematic_contract::operations::Operations;

/* === Constants === */
pub const NAME: &str = "schematic-tools";

const SDK_ENCODING_MEMORY_BYTES: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/* === Macros === */

/* === Type Aliases === */
pub type Manager = PluginManager<Guest, (), ()>;
type Backend = PluginBackend<Guest, (), ()>;

/* === Structs === */
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
    operations: harmos::operations::OperationManager,
    guest: Option<Guest>,
}

/* === Enums === */
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
                field: "schematic tools artifact",
                reason: "this lifecycle owns the inspected Schematic guest",
            });
        }
        let guest = self.guest.take().ok_or(harmos::Error::InvalidInput {
            field: "schematic tools generation",
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
        let registered = registered.routed(router).map_err(failure)?;
        guest.serving = Some(registered.serve_sdk().await.map_err(failure)?);
        let serving = guest.serving.as_ref().unwrap();
        let component = Arc::new(serving.operations().map_err(failure)?);
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

/* === Public Functions === */
pub async fn lifecycle(
    runtime: &Runtime,
    loaded: Loaded<SchematicEditor>,
) -> harmos::Result<(Manager, Lifecycle)> {
    let declaration = loaded.declaration().clone();
    if declaration.id != NAME {
        return Err(harmos::Error::InvalidInput {
            field: "schematic tools artifact",
            reason: "the inspected component has a different owner",
        });
    }
    let contracts = &declaration.contracts;
    let sha256 = loaded.sha256();
    let registered = loaded.register().await.map_err(failure)?;
    let definition = PluginDefinition {
        id: declaration.id.clone(),
        version: declaration.version,
        artifact: PluginArtifact::Guest { sha256 },
        requires: Vec::new(),
        configuration: b"null".to_vec(),
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

pub fn load(
    runtime: &Runtime,
    component: &[u8],
) -> Result<Loaded<SchematicEditor>, harmos::plugin::guest::Error> {
    let host = Host::new(runtime)?;
    host.load(component)
}

/* === Private Functions === */
pub(super) fn failure(source: impl std::error::Error + Send + Sync + 'static) -> harmos::Error {
    harmos::Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
