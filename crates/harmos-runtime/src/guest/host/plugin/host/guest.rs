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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Execution, Factory, LifecycleFuture, PluginHost};
use crate::config::PluginSettings;
use crate::guest::host::plugin::{PluginDefinition, PluginLifecycleAuthority, Router};
use crate::guest::{Loaded, Serving};
use crate::operations::{EncodedInvocation, Lane, Manager, PluginRegistration, PluginSdkRegistration};
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Guest<A: Application> {
    loaded: Option<Loaded<A>>,
    serving: Option<Serving<A>>,
    exited: Option<LifecycleFuture<'static>>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    operations: Manager,
    initialization_timeout: Duration,
    shutdown_timeout: Duration,
    encoding_memory_bytes: NonZeroUsize,
    lane: Lane,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> PluginHost<A> {
    pub async fn register_guest(&self, loaded: Loaded<A>, settings: PluginSettings, lane: Lane) -> Result<()> {
        self.guest(loaded, settings, lane, false).await
    }

    pub async fn reload_guest(&self, loaded: Loaded<A>, settings: PluginSettings, lane: Lane) -> Result<()> {
        self.guest(loaded, settings, lane, true).await
    }

    async fn guest(&self, loaded: Loaded<A>, settings: PluginSettings, lane: Lane, replace: bool) -> Result<()> {
        settings.validate()?;
        loaded.validate_publications().map_err(|source| failed(&loaded.declaration().id, source))?;
        let mut definition = PluginDefinition::guest(&loaded, settings.configuration.clone())
            .map_err(|source| failed(loaded.declaration().id.as_str(), source))?;
        definition.requires = settings.requires.clone();
        for grant in &settings.grants {
            let required = definition
                .contributions
                .required_contracts
                .as_mut()
                .and_then(|requirements| {
                    requirements.iter_mut().find(|required| required.contract.operation().id.as_str() == grant.operation())
                })
                .ok_or(Error::InvalidInput {
                    field: "guest grants",
                    reason: "grant must name a declared external requirement",
                })?;
            if !required.grants.contains(grant) {
                required.grants.push(grant.clone());
            }
        }
        let grants = settings.host_operations.iter().cloned().collect();
        let operations = self.shared.operations.clone();
        let initialization_timeout = settings.initialization_timeout;
        let shutdown_timeout = settings.shutdown_timeout;
        let encoding_memory_bytes = settings.encoding_memory_bytes;
        let factory: Factory = Arc::new(move || {
            let inspected = loaded.duplicate().map_err(|source| failed(&loaded.declaration().id, source))?;
            Ok(Box::new(Guest::<A> {
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
                lane,
            }))
        });
        self.register(definition, factory, grants, settings.restart, replace).await
    }
}

impl<A: Application> Execution for Guest<A> {
    fn start<'a>(
        &'a mut self, definition: &'a PluginDefinition, router: &'a Router, authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let timeout = self.initialization_timeout;
            let starting = async {
                let loaded = self.loaded.take().expect("one inspected guest per attempt");
                let registered = loaded.register().await.map_err(|source| failed(&definition.id, source))?;
                let registered = registered.routed(router).map_err(|source| failed(&definition.id, source))?;
                self.serving = Some(registered.serve_sdk().await.map_err(|source| failed(&definition.id, source))?);
                let serving = self.serving.as_ref().unwrap();
                let exited = serving.exit().map_err(|source| failed(&definition.id, source))?;
                let owner = definition.id.clone();
                self.exited = Some(Box::pin(async move { exited.await.map_err(|source| failed(&owner, source)) }));
                let component = Arc::new(serving.operations().map_err(|source| failed(&definition.id, source))?);
                self.registration =
                    Some(self.operations.stage_plugin(serving.scope().unwrap(), |bindings| component.register(bindings, self.lane))?);
                self.sdk = Some(component.stage_sdk(&self.operations, self.lane)?);
                self.initializing =
                    Some(self.sdk.as_ref().unwrap().submit_initialize(authority, &definition.configuration, self.encoding_memory_bytes)?);
                let result = self.initializing.as_mut().unwrap().await;
                self.initializing.take();
                result?;
                self.initialized = true;
                Ok(())
            };
            crate::supervision::executor::timeout(timeout, starting).await.map_err(|_| Error::NotReady {
                domain: "guest plugin initialization timeout",
            })?
        })
    }

    fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<()>> {
        self.exited.as_mut().map_or(Poll::Pending, |exited| exited.as_mut().poll(context))
    }

    fn stop<'a>(&'a mut self, _: &'a PluginDefinition, authority: &'a PluginLifecycleAuthority<'a>) -> LifecycleFuture<'a> {
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
                .map_err(|_| Error::NotReady { domain: "guest plugin shutdown timeout" })?
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
