/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive Guest SDK Lifecycle And Cleanup Behind The Runtime Plugin Owner.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Execution, Factory, LifecycleFuture, PluginHost};
use crate::catalog::Catalog;
use crate::config::PluginSettings;
use crate::operations::{
    ComponentBindings, Declarations, EncodedInvocation, Lane, OperationManager, PluginRegistration,
    PluginSdkRegistration,
};
use crate::plugin::guest::{Loaded, Serving};
use crate::plugin::{PluginDefinition, PluginLifecycleAuthority, Router};
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Guest<A: Application, C, L> {
    loaded: Option<Loaded<A>>,
    serving: Option<Serving<A>>,
    exited: Option<LifecycleFuture<'static>>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    operations: OperationManager,
    initialization_timeout: Duration,
    shutdown_timeout: Duration,
    encoding_memory_bytes: NonZeroUsize,
    lane: L,
    catalog: PhantomData<fn() -> C>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> PluginHost<A> {
    pub async fn register_guest<C>(
        &self,
        loaded: Loaded<A>,
        settings: PluginSettings,
        lane: impl Lane + Clone + Sync,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<ComponentBindings> + 'static,
    {
        self.guest::<C>(loaded, settings, lane, false).await
    }

    pub async fn reload_guest<C>(
        &self,
        loaded: Loaded<A>,
        settings: PluginSettings,
        lane: impl Lane + Clone + Sync,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<ComponentBindings> + 'static,
    {
        self.guest::<C>(loaded, settings, lane, true).await
    }

    async fn guest<C>(
        &self,
        loaded: Loaded<A>,
        settings: PluginSettings,
        lane: impl Lane + Clone + Sync,
        replace: bool,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<ComponentBindings> + 'static,
    {
        settings.validate()?;
        let mut definition = PluginDefinition::guest(&loaded, settings.configuration.clone())
            .map_err(|source| failed(loaded.declaration().id.as_str(), source))?;
        definition.requires = settings.requires.clone();
        let declarations =
            Declarations::of::<C>().map_err(|source| failed(&definition.id, source))?;
        harmos_contract::inventory::operations::validate_match(
            &declarations,
            &definition.contributions.operations,
        )
        .map_err(|_| Error::InvalidInput {
            field: "guest operation catalog",
            reason: "catalog must match the inspected component",
        })?;
        let grants = settings.host_operations.iter().cloned().collect();
        let operations = self.shared.operations.clone();
        let initialization_timeout = settings.initialization_timeout;
        let shutdown_timeout = settings.shutdown_timeout;
        let encoding_memory_bytes = settings.encoding_memory_bytes;
        let factory: Factory = Arc::new(move || {
            let inspected = loaded
                .duplicate()
                .map_err(|source| failed(&loaded.declaration().id, source))?;
            Ok(Box::new(Guest::<A, C, _> {
                loaded: Some(inspected),
                serving: None,
                exited: None,
                registration: None,
                sdk: None,
                initializing: None,
                shutting_down: None,
                initialized: false,
                shutdown_attempted: false,
                operations: operations.clone(),
                initialization_timeout,
                shutdown_timeout,
                encoding_memory_bytes,
                lane: lane.clone(),
                catalog: PhantomData,
            }))
        });
        self.register(definition, factory, grants, settings.restart, replace)
            .await
    }
}

impl<A, C, L> Execution for Guest<A, C, L>
where
    A: Application,
    C: Catalog<Declarations> + Catalog<ComponentBindings> + 'static,
    L: Lane + Clone + Sync,
{
    fn start<'a>(
        &'a mut self,
        definition: &'a PluginDefinition,
        router: &'a Router,
        authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let timeout = self.initialization_timeout;
            let starting = async {
                let loaded = self.loaded.take().expect("one inspected guest per attempt");
                let registered = loaded
                    .register()
                    .await
                    .map_err(|source| failed(&definition.id, source))?;
                let mut collected = definition.clone();
                collected.contributions.registrations = registered.registrations().to_vec();
                collected
                    .validate()
                    .map_err(|source| failed(&definition.id, source))?;
                let registered = registered
                    .routed(router)
                    .map_err(|source| failed(&definition.id, source))?;
                self.serving = Some(
                    registered
                        .serve_sdk()
                        .await
                        .map_err(|source| failed(&definition.id, source))?,
                );
                let serving = self.serving.as_ref().unwrap();
                let exited = serving
                    .exit()
                    .map_err(|source| failed(&definition.id, source))?;
                let owner = definition.id.clone();
                self.exited = Some(Box::pin(async move {
                    exited.await.map_err(|source| failed(&owner, source))
                }));
                let component = Arc::new(
                    serving
                        .operations()
                        .map_err(|source| failed(&definition.id, source))?,
                );
                self.registration = Some(self.operations.stage_plugin(
                    serving.scope().unwrap(),
                    |bindings| {
                        bindings
                            .register_component_catalog::<C>(self.lane.clone(), component.clone())
                    },
                )?);
                self.sdk = Some(component.stage_sdk(&self.operations, self.lane.clone())?);
                self.initializing = Some(self.sdk.as_ref().unwrap().submit_initialize(
                    authority,
                    &definition.configuration,
                    self.encoding_memory_bytes,
                )?);
                let result = self.initializing.as_mut().unwrap().await;
                self.initializing.take();
                result?;
                self.initialized = true;
                Ok(())
            };
            crate::supervision::executor::timeout(timeout, starting)
                .await
                .map_err(|_| Error::NotReady {
                    domain: "guest plugin initialization timeout",
                })?
        })
    }

    fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<()>> {
        self.exited
            .as_mut()
            .map_or(Poll::Pending, |exited| exited.as_mut().poll(context))
    }

    fn stop<'a>(
        &'a mut self,
        _: &'a PluginDefinition,
        authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let stopping = async {
                if let Some(sdk) = &self.sdk {
                    sdk.settle().await?;
                    if let Some(initializing) = &mut self.initializing {
                        let result = initializing.await;
                        self.initializing.take();
                        result?;
                        self.initialized = true;
                    }
                    if self.initialized && !self.shutdown_attempted {
                        self.shutting_down = Some(sdk.submit_shutdown(authority)?);
                        self.shutdown_attempted = true;
                    }
                    if let Some(shutdown) = &mut self.shutting_down {
                        let result = shutdown.await;
                        self.shutting_down.take();
                        result?;
                        self.initialized = false;
                    }
                }
                if let Some(registration) = &mut self.registration {
                    registration.shutdown().await?;
                }
                if let Some(serving) = &mut self.serving {
                    serving.finalize(self.shutdown_timeout).await?;
                }
                if let Some(sdk) = &mut self.sdk {
                    sdk.close().await?;
                }
                self.sdk.take();
                self.registration.take();
                self.serving.take();
                self.exited.take();
                self.loaded.take();
                Ok(())
            };
            // A Timed-Out Wait Retains SDK Invocations And Physical Cleanup Owners.
            crate::supervision::executor::timeout(self.shutdown_timeout, stopping)
                .await
                .map_err(|_| Error::NotReady {
                    domain: "guest plugin shutdown timeout",
                })?
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn failed(id: &str, source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: id.to_owned(),
        source: Arc::new(source),
    }
}

/* === Tests === */
