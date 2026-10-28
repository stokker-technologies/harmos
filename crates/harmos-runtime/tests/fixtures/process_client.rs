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
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::contract;
use crate::sidecar::witness::Witness;
use contract::{POLICY, ProbeDeclaration, ReadDeclaration, Readings, Step};
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::OperationPolicy;
use harmos_runtime::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Client {
    resources: Arc<Witness>,
}

#[harmos_runtime::operation(codec = manual, resources = [Witness], id = <ProbeDeclaration as harmos_runtime::OperationDeclaration>::ID, response = Step,
    policy = POLICY, visibility = Host)]
#[derive(Clone)]
struct Probing(ProbeDeclaration);

impl harmos_runtime::Message for Probing {
    const CODEC: &'static str = <ProbeDeclaration as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <ProbeDeclaration as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <ProbeDeclaration as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Probing {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Probing {
    const NAME: &'static str = <ProbeDeclaration as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <ProbeDeclaration as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <ProbeDeclaration as harmos_runtime::schema::Described>::MARKER;
}

struct AttemptGuard {
    resources: Arc<Witness>,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Handlers {
    Probe(Probing),
}

/* === Traits === */

/* === Implementations === */

impl harmos_operations::lifecycle::Policy for Client {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

impl harmos_runtime::Lifecycle for Client {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;

    async fn prepare(key: String, setup: &mut harmos_sdk::Setup<Self>) -> std::result::Result<Self, Refusal> {
        let plugin = Self {
            resources: Witness::find(&key).ok_or_else(|| Refusal::of("the story witness is gone"))?,
        };
        setup.resource::<Witness>(Arc::clone(&plugin.resources));
        Ok(plugin)
    }

    async fn initialize(&mut self, context: &Supervisor) -> std::result::Result<(), Refusal> {
        self.resources.mark("initializing");
        let response = context.call::<contract::ReadDeclaration>(ReadDeclaration { phase: 1 }).await.map_err(Refusal::from)?;
        if response.phase != 1 {
            return Err(Refusal::of("initialization used another provider response"));
        }
        self.resources.mark("dependency-initialized");
        if self.resources.seen("hold-initialization") {
            self.resources.marked("release-initialization").await;
        }
        self.resources.mark("initialized");
        Ok(())
    }

    async fn stop(&mut self, context: &Supervisor) -> std::result::Result<(), Refusal> {
        let response = context.call::<contract::ReadDeclaration>(ReadDeclaration { phase: 2 }).await.map_err(Refusal::from)?;
        if response.phase != 2 {
            return Err(Refusal::of("shutdown used another provider response"));
        }
        self.resources.mark("dependency-shutdown");
        self.resources.mark("shutdown");
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Refusal> {
        self.resources.mark("cleaned");
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "process_client", requires = Readings)]
impl harmos_runtime::Plugin for Client {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Handlers::CATALOG];
}

impl harmos_runtime::Operation for Probing {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Step>, Refusal> {
        let resources = ctx.resource::<Witness>();
        let request = self.0;
        resources.mark("probe-initialized");
        let _guard = AttemptGuard { resources: resources.clone() };
        let result: ::core::result::Result<Step, Self::Error> = async move {
            resources.mark("probe-started");
            Ok(Step { phase: request.phase })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.resources.mark("probe-finalized");
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
