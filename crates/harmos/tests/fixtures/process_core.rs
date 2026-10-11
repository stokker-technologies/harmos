/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/process_core_contract.rs"]
mod contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use contract::{Inspect, POLICY, Queries};
use harmos::operations::{Operation, OperationPolicy, WireTable, host};
use harmos::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Process {
    resources: Arc<Resources>,
}

struct Resources {
    root: PathBuf,
}

struct Inspecting {
    _resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Resources {
    fn mark(&self, name: &str) -> Result<(), Refusal> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join(name))
            .map_err(Refusal::of)?;
        file.write_all(b"1\n").map_err(Refusal::of)
    }
}

impl harmos::operations::lifecycle::Policy for Process {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(
    id = "process-core",
    provides = Queries,
    requires(host::Egress: egress("vendor"))
)]
impl harmos::Plugin for Process {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Resources;
    fn prepare(root: String) -> Result<Self, Refusal> {
        let resources = Arc::new(Resources { root: root.into() });
        resources.mark("prepared")?;
        Ok(Self { resources })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.mark("initialized")
    }
    fn resources(&self) -> Arc<Resources> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<Inspecting>(self.resources())
    }
    async fn shutdown(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.mark("shutdown")
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.mark("cleaned")
    }
}

impl Operation for Inspecting {
    type Declaration = Inspect;
    type Resources = Resources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    fn new(resources: Arc<Resources>) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        name: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Refusal> {
        let answered = context
            .operation::<host::Egress>(host::EgressRequest {
                name: name.clone(),
                method: "get".into(),
                path: "/reading".into(),
                headers: vec![],
                body: vec![],
            })
            .await;
        Ok(match answered {
            Ok(response) => format!("egress status {}", response.status),
            Err(error) => error.to_string(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(harmos::sidecar::run_plugin::<Process>(
            NonZeroUsize::new(2).unwrap(),
        ))
        .unwrap();
}

/* === Tests === */
