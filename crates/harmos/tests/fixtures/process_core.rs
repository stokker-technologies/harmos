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
use harmos::operations::Host as _;
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
    requires(host::Egress: scope("vendor"))
)]
impl harmos::Plugin for Process {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Resources;
    fn prepare(root: String) -> Result<Self, Refusal> {
        let resources = Arc::new(Resources { root: root.into() });
        resources.mark("prepared")?;
        if let Ok(failures) = std::env::var("HARMOS_TEST_CORE_CRASH_STARTS") {
            let failures: usize = failures.parse().map_err(Refusal::of)?;
            resources.mark("attempts")?;
            let attempts = std::fs::read_to_string(resources.root.join("attempts"))
                .map_err(Refusal::of)?
                .lines()
                .count();
            if attempts <= failures {
                std::process::exit(75);
            }
        }
        Ok(Self { resources })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        if self.resources.root.join("hold-initialize").exists() {
            self.resources.mark("initializing")?;
            while self.resources.root.join("hold-initialize").exists() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }
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
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.mark("shutdown")?;
        if std::env::var("HARMOS_TEST_CORE_FAIL_SHUTDOWN").as_deref() == Ok("1") {
            return Err(Refusal::of(std::io::Error::other(
                "controlled shutdown failure",
            )));
        }
        Ok(())
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
    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        name: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            if name == "crash" {
                std::process::exit(75);
            }
            let selected = if name.starts_with("context") {
                let parent = context
                    .context()
                    .value::<contract::Stage>()
                    .map_err(Refusal::of)?
                    .unwrap();
                assert_eq!(parent.id, "parent");
                context
                    .with_value(contract::Stage { id: "child".into() })
                    .map_err(Refusal::of)?
            } else {
                context.clone()
            };
            let answered = selected
                .call_as::<host::Egress>(host::EgressRequest {
                    name: if name == "context-denied" {
                        "other".into()
                    } else if name.starts_with("context") {
                        "vendor".into()
                    } else {
                        name.clone()
                    },
                    method: "get".into(),
                    path: if name == "context-wait" {
                        "/context-wait".into()
                    } else if name.starts_with("context") {
                        "/context".into()
                    } else {
                        "/reading".into()
                    },
                    headers: vec![],
                    body: vec![],
                })
                .await;
            Ok(match answered {
                Ok(response) if name == "context" => {
                    let parent = context
                        .context()
                        .value::<contract::Stage>()
                        .map_err(Refusal::of)?
                        .unwrap();
                    let child =
                        String::from_utf8(response.into_owned().body).map_err(Refusal::of)?;
                    format!("{}:{child}", parent.id)
                }
                Ok(response) => format!("egress status {}", response.status),
                Err(error) => error.to_string(),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
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
