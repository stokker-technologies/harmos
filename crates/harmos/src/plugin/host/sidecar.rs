/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive Native SDK Lifecycle Behind The Shared Runtime Plugin Owner.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::task::{Context, Poll};

// Third Party Imports

// Local Imports
use super::{Execution, Factory, LifecycleFuture, PluginHost};
use crate::catalog::Catalog;
use crate::config::PluginSidecarSettings;
use crate::operations::{Declarations, Lane, NativeBindings};
use crate::plugin::sidecar::{Executable, Notice, Supervised};
use crate::plugin::{PluginDefinition, PluginLifecycleAuthority, Router};
use crate::services::{RuntimeDriver, Service};
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Native<A: Application, L> {
    driver: RuntimeDriver<A, L>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> PluginHost<A> {
    pub async fn register_sidecar<C>(
        &self,
        executable: Executable,
        settings: PluginSidecarSettings,
        lane: impl Lane + Clone + Sync,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<NativeBindings> + 'static,
    {
        self.sidecar::<C>(executable, settings, lane, false).await
    }

    pub async fn reload_sidecar<C>(
        &self,
        executable: Executable,
        settings: PluginSidecarSettings,
        lane: impl Lane + Clone + Sync,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<NativeBindings> + 'static,
    {
        self.sidecar::<C>(executable, settings, lane, true).await
    }

    async fn sidecar<C>(
        &self,
        executable: Executable,
        settings: PluginSidecarSettings,
        lane: impl Lane + Clone + Sync,
        replace: bool,
    ) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<NativeBindings> + 'static,
    {
        settings.plugin.validate()?;
        let mut definition =
            PluginDefinition::sidecar(&executable, settings.plugin.configuration.clone())
                .map_err(|source| failed(executable.declaration().id.as_str(), source))?;
        definition.requires = settings.plugin.requires.clone();
        let declarations =
            Declarations::of::<C>().map_err(|source| failed(&definition.id, source))?;
        harmos_contract::inventory::operations::validate_match(
            &declarations,
            &definition.contributions.operations,
        )
        .map_err(|_| Error::InvalidInput {
            field: "native plugin catalog",
            reason: "typed catalog differs from the inspected operation inventory",
        })?;
        let grants = settings.plugin.host_operations.iter().cloned().collect();
        let restart = settings.plugin.restart;
        let handles = self.shared.handles.clone();
        let operations = self.shared.operations.clone();
        let driving = self.shared.driving.clone();
        let frozen = definition.clone();
        let factory: Factory = Arc::new(move || {
            let resident = Supervised::new(
                executable.clone(),
                diagnostics::<A> as fn(&Service<A>, Notice),
            )
            .handshake(settings.plugin.initialization_timeout)
            .grace(settings.plugin.shutdown_timeout);
            let driver = RuntimeDriver::new::<C>(
                resident,
                &frozen,
                operations.clone(),
                handles.streams.clone(),
                lane.clone(),
                settings.calls,
                settings.plugin.encoding_memory_bytes,
                driving.clone(),
            )?;
            Ok(Box::new(Native { driver }))
        });
        self.register(definition, factory, grants, restart, replace)
            .await
    }
}

impl<A, L> Execution for Native<A, L>
where
    A: Application,
    L: Lane + Clone,
{
    fn start<'a>(
        &'a mut self,
        definition: &'a PluginDefinition,
        router: &'a Router,
        authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(self.driver.start(definition, router, authority))
    }

    fn stop<'a>(
        &'a mut self,
        definition: &'a PluginDefinition,
        authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(self.driver.stop(definition, authority))
    }

    fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<()>> {
        self.driver.poll(context)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn diagnostics<A: Application>(_: &Service<A>, notice: Notice) {
    if let Notice::Logged { line } = notice {
        tracing::debug!(target: "harmos::plugin::sidecar", %line, "native plugin output");
    }
}

fn failed(owner: &str, source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: owner.to_owned(),
        source: Arc::new(source),
    }
}

/* === Tests === */
