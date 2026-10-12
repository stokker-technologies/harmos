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
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::contract;
use contract::{Marks, POLICY, Probe, Queries, Read, Readings, Step};
use harmos::operations::{Operation, OperationPolicy, WireTable};
use harmos::plugin::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Client {
    resources: Arc<Marks>,
}

struct Probing {
    resources: Arc<Marks>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Client {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "process-client", provides = Queries, requires = Readings)]
impl harmos::Plugin for Client {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Marks;

    fn prepare(root: String) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Marks { root: root.into() }),
        })
    }

    async fn initialize(&mut self, context: &Supervisor) -> Result<(), Refusal> {
        std::fs::create_dir_all(&self.resources.root).map_err(Refusal::of)?;
        self.resources.mark("initializing")?;
        let response = context
            .operation::<Read>(Step { phase: 1 })
            .await
            .map_err(Refusal::of)?;
        if response.phase != 1 {
            return Err(Refusal::of("initialization used another provider response"));
        }
        self.resources.mark("dependency-initialized")?;
        if self.resources.root.join("hold-initialization").exists() {
            while !self.resources.root.join("release-initialization").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        self.resources.mark("initialized")
    }

    fn resources(&self) -> Arc<Marks> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<Probing>(self.resources())
    }

    async fn shutdown(&mut self, context: &Supervisor) -> Result<(), Refusal> {
        let response = context
            .operation::<Read>(Step { phase: 2 })
            .await
            .map_err(Refusal::of)?;
        if response.phase != 2 {
            return Err(Refusal::of("shutdown used another provider response"));
        }
        self.resources.mark("dependency-shutdown")?;
        self.resources.mark("shutdown")
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.mark("cleaned")
    }
}

impl Operation for Probing {
    type Declaration = Probe;
    type Resources = Marks;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;

    fn new(resources: Arc<Marks>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }

    async fn initialize<'ctx>(&mut self, _: Self::Context<'ctx>) -> Result<(), Refusal> {
        self.resources.mark("probe-initialized")
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Step,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Step, Refusal> {
        self.resources.mark("probe-started")?;
        Ok(request.clone())
    }

    async fn finalize(&mut self) -> Result<(), Refusal> {
        self.resources.mark("probe-finalized")
    }
}

/* === Public Functions === */

/* === Private Functions === */
/* === Tests === */
