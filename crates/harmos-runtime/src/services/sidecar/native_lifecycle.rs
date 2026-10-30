/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use std::num::NonZeroUsize;

use crate::operations::{Bindings, EncodedInvocation, Lane, Manager, Native, PluginRegistration, PluginSdkRegistration};
use crate::plugin::{PluginLifecycleAuthority, PluginScope};
use crate::{Error, Result};

/* === Structs === */

/// Retains one native generation's registrations and lifecycle receipts.
/// The caller owns physical progress, shutdown deadlines and process settlement.
#[derive(Default)]
pub struct NativeLifecycle {
    scope: Option<PluginScope>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: Option<tokio::sync::watch::Receiver<bool>>,
    shutdown_attempted: bool,
}

/* === Implementations === */

impl NativeLifecycle {
    pub fn scope(&self) -> Option<&PluginScope> {
        self.scope.as_ref()
    }

    pub fn stage_operations(
        &mut self, native: &Native, operations: &Manager, bind: impl FnOnce(&mut Bindings<'_>) -> Result<()>,
    ) -> Result<()> {
        let scope = native.scope().ok_or(Error::InvalidInput {
            field: "native lifecycle",
            reason: "native endpoint has no admitted plugin generation",
        })?;
        self.scope = Some(scope.clone());
        self.registration = Some(operations.stage_plugin(scope, bind)?);
        self.initialized = Some(native.initialization());
        Ok(())
    }

    pub fn stage_sdk(&mut self, native: &Native, operations: &Manager, lane: impl Lane) -> Result<()> {
        self.sdk = Some(native.stage_sdk(operations, lane)?);
        Ok(())
    }

    pub fn begin_initialize(
        &mut self, authority: &PluginLifecycleAuthority<'_>, configuration: &[u8], encoding_memory_bytes: NonZeroUsize,
    ) -> Result<()> {
        self.initializing = Some(self.sdk.as_ref().ok_or(Error::NotReady { domain: "native SDK lifecycle" })?.submit_initialize(
            authority,
            configuration,
            encoding_memory_bytes,
        )?);
        Ok(())
    }

    pub async fn settle(&mut self) -> Result<()> {
        if let Some(sdk) = &self.sdk {
            sdk.settle().await?;
        }
        Ok(())
    }

    pub async fn join_initialization(&mut self) -> Result<()> {
        if let Some(initializing) = &mut self.initializing {
            let outcome = initializing.await;
            self.initializing.take();
            outcome?;
        }
        Ok(())
    }

    pub fn begin_shutdown(&mut self, authority: &PluginLifecycleAuthority<'_>, running: bool) -> Result<()> {
        let initialized = self.initialized.as_ref().is_some_and(|original| *original.borrow());
        if let Some(sdk) = &self.sdk
            && running
            && initialized
            && !self.shutdown_attempted
        {
            self.shutting_down = Some(sdk.submit_shutdown(authority)?);
            self.shutdown_attempted = true;
        }
        Ok(())
    }

    pub fn shutdown_pending(&self) -> bool {
        self.shutting_down.is_some()
    }

    pub async fn join_shutdown(&mut self) -> Result<()> {
        if let Some(shutdown) = &mut self.shutting_down {
            let outcome = shutdown.await;
            self.shutting_down.take();
            outcome?;
        }
        Ok(())
    }

    pub async fn withdraw(&mut self) -> Result<()> {
        if let Some(registration) = &mut self.registration {
            registration.shutdown().await?;
        }
        self.registration.take();
        Ok(())
    }

    /// Close SDK entries after physical settlement; failed closes remain owned for retry.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(sdk) = &mut self.sdk {
            sdk.close().await?;
        }
        self.sdk.take();
        self.scope.take();
        self.initialized.take();
        Ok(())
    }
}
