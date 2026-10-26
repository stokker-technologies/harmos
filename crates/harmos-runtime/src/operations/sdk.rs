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
// Third Party Imports

// Local Imports
use super::caller::Source;
use super::manager::{Entry, Submission};
use super::wire::EncodedInput;
use super::{EncodedInvocation, Manager};
use crate::operations::lifecycle::InitializeRequest;
use crate::plugin::{PluginKind, PluginLifecycleAuthority, PluginLifecyclePhase, PluginScope};

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
use crate::operations::{OperationDeclaration, OperationPolicy, RetryPolicy};
use crate::supervision::executor;
use crate::{Error, Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Erased Lifecycle Request Forwarded To A Guest As Its Original Bytes.
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub(super) struct Encoded(pub(super) Vec<u8>);

#[must_use = "retain SDK entries through original invocation settlement"]
pub struct PluginSdkRegistration {
    operations: Manager,
    entries: [Arc<Entry>; 2],
    kind: PluginKind,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
impl crate::Message for Encoded {
    const CODEC: &'static str = "harmos.raw.v1";
    fn encode(&self) -> Vec<u8> {
        self.0.clone()
    }

    fn encoded_size(&self) -> usize {
        self.0.len()
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
        std::mem::size_of::<Self>()
            .checked_add(bytes.len())
            .ok_or_else(|| crate::MessageError::borrowed("forwarded request size overflows"))
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
        Ok(Self(bytes.to_vec()))
    }
}

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
impl crate::schema::Described for Encoded {
    const NAME: &'static str = "harmos::sdk_encoded";
    const FIELDS: &'static [crate::schema::declaration::Field] = &[];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
impl crate::MemorySize for Encoded {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.0.capacity())
    }
}

// This Adapter Is Never Declared Or Registered By Its Static Identity.
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
impl OperationDeclaration for Encoded {
    type Request = Self;
    type Response = Vec<u8>;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "harmos.encoded";
    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: harmos_domain::operations::UNBOUNDED,
        attempt_timeout: harmos_domain::operations::UNBOUNDED,
        retries: RetryPolicy::Never,
    };
}

impl PluginSdkRegistration {
    pub(super) fn new(operations: Manager, scope: &PluginScope, mut entries: [Arc<Entry>; 2], kind: PluginKind) -> Result<Self> {
        scope.issue_generation_context()?;
        let pair = scope.lifecycle().ok_or(Error::InvalidInput { field: "SDK lifecycle", reason: "generation has no frozen SDK pair" })?;
        for (entry, descriptor) in entries.iter_mut().zip(pair) {
            let selected =
                entry.plugin().ok_or(Error::InvalidInput { field: "SDK lifecycle", reason: "entry has no original generation" })?;
            if !selected.same_activation(scope) || entry.contract.as_ref() != Some(descriptor.contract()) {
                return Err(Error::InvalidInput {
                    field: "SDK lifecycle",
                    reason: "entry differs from the frozen SDK descriptor or generation",
                });
            }
            let action = descriptor.action();
            if entry.declared() != descriptor.contract().operation() {
                return Err(Error::InvalidInput { field: "SDK lifecycle", reason: "entry differs from the SDK definition" });
            }
            Arc::get_mut(entry).expect("new SDK entry has one owner").sdk = Some(action);
        }
        operations.reserve(&entries)?;
        Ok(Self { operations, entries, kind })
    }

    pub fn submit_initialize(
        &self, authority: &PluginLifecycleAuthority<'_>, config_json: &[u8], encoding_memory_bytes: NonZeroUsize,
    ) -> Result<EncodedInvocation> {
        self.authorize(authority, 0)?;
        let generation_context = self.entries[0].plugin().expect("SDK retains its original scope").issue_generation_context()?;
        harmos_domain::validate_config(config_json)
            .map_err(|source| Error::OperationFailed { operation: self.entries[0].id().to_owned(), source: Arc::new(source) })?;
        self.submit(0, encoding_memory_bytes, |allowance| {
            let peak = config_json
                .len()
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<InitializeRequest>() + 64 + 30))
                .ok_or(Error::ResourceExhausted { resource: "SDK encoding memory" })?;
            if peak > allowance {
                return Err(Error::ResourceExhausted { resource: "SDK encoding memory" });
            }
            let scope = self.entries[0].plugin().expect("SDK retains its original scope");
            let assignments = self.operations.encode_assignments(scope, allowance - peak)?;
            let request =
                InitializeRequest { config_json: config_json.to_vec(), generation_context: generation_context.to_vec(), assignments };
            Ok(request.encode())
        })
    }

    pub fn submit_shutdown(&self, authority: &PluginLifecycleAuthority<'_>) -> Result<EncodedInvocation> {
        self.authorize(authority, 1)?;
        self.submit(1, NonZeroUsize::new(1).unwrap(), |_| Ok(Vec::new()))
    }

    fn authorize(&self, authority: &PluginLifecycleAuthority<'_>, index: usize) -> Result<()> {
        let entry = &self.entries[index];
        let scope = entry.plugin().expect("SDK entries retain their original scope");
        let phase = if index == 0 { PluginLifecyclePhase::Initialize } else { PluginLifecyclePhase::Shutdown };
        if !authority.matches_scope(scope, self.kind, phase)
            || authority.declaration().map(|descriptor| descriptor.contract()) != entry.contract.as_ref()
        {
            return Err(Error::InvalidInput {
                field: "SDK lifecycle authority",
                reason: "current hook loan differs from this original SDK action",
            });
        }
        Ok(())
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.submit", level = "debug", skip_all)]
    fn submit(&self, index: usize, bytes: NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>) -> Result<EncodedInvocation> {
        let submitted = executor::now();
        let entry = &self.entries[index];
        entry.available()?;
        if !entry.settled() {
            return Err(Error::ResourceExhausted { resource: "SDK physical settlement" });
        }
        let preparation = self.operations.claim_preparation(bytes)?;
        #[cfg(feature = "plugin-sidecar")]
        let native_capacity = entry
            .native
            .as_ref()
            .map(|binding| binding.reserve())
            .transpose()
            .map_err(|source| Error::OperationFailed { operation: entry.id().to_owned(), source: Arc::new(source) })?;
        let payload = encode(bytes.get())?;
        if payload.capacity() > bytes.get() {
            return Err(Error::ResourceExhausted { resource: "SDK encoding memory" });
        }
        let dispatch = entry.wire.ok_or(Error::InvalidInput { field: "SDK lifecycle", reason: "entry has no original wire adapter" })?;
        let mut invocation = dispatch(
            &self.operations,
            Submission { submitted, entry: Arc::clone(entry), cause: None, preparation: None },
            Source::Host,
            None,
            EncodedInput {
                payload,
                preparation: Some(preparation),
                #[cfg(feature = "plugin-sidecar")]
                native_capacity,
            },
        )?;
        let outcome = invocation.outcome;
        let control = Arc::clone(&invocation.control);
        invocation.outcome = Box::pin(async move {
            let payload = outcome.await?;
            if !payload.is_empty() {
                return Err(Error::InvalidInput { field: "SDK response", reason: "expected the exact empty SDK acknowledgement" });
            }
            control.settle().await.map_err(|source| Error::OperationSettlement { source })?;
            Ok(payload)
        });
        Ok(invocation)
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.settle", level = "debug", skip_all)]
    pub async fn settle(&self) -> Result<()> {
        self.operations.settle_owned_entries(&self.entries).await?;
        #[cfg(feature = "plugin-sidecar")]
        if self.entries[0].plugin().is_some_and(PluginScope::is_closed)
            && let Some(binding) = &self.entries[0].native
        {
            binding.settle_observers().await?;
        }
        Ok(())
    }

    pub fn observer_outcome(&self) -> Result<()> {
        #[cfg(feature = "plugin-sidecar")]
        if let Some(binding) = &self.entries[0].native {
            binding.observer_outcome()?;
        }
        Ok(())
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.close", level = "debug", skip_all)]
    pub async fn close(&mut self) -> Result<()> {
        self.operations.fence(&self.entries, false);
        self.operations.settle_entries(&self.entries).await
    }
}

impl Drop for PluginSdkRegistration {
    fn drop(&mut self) {
        self.operations.fence(&self.entries, true);
        self.operations.remove_settled(&self.entries);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
