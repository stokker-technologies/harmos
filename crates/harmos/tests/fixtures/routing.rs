/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/process_contract.rs"]
mod contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports

// Local Imports
use contract::{Blob, Echo, Nested, Operations, POLICY, Probe};
use harmos::operations::{Operation, OperationPolicy, WireTable};
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
    probes: AtomicU64,
}

struct Echoing {
    resources: Arc<Resources>,
    id: Option<u64>,
    run_started: bool,
}

struct Probing {
    _resources: Arc<Resources>,

    sequence: u64,
}

struct Publishing {
    resources: Arc<Resources>,
    id: Option<u64>,
}

struct Work<'a> {
    resources: &'a Resources,
    id: u64,
}

/* === Enums === */
#[harmos::catalog]
enum Implemented {
    Echo(Echoing),
    Probe(Probing),
    Nested(Publishing),
}

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

impl Drop for Work<'_> {
    fn drop(&mut self) {
        let _ = self.resources.mark(&format!("echo-dropped-{}", self.id));
    }
}

impl harmos::operations::lifecycle::Policy for Process {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "process", provides = Operations, requires(harmos::operations::streams::Publish),  streams(Blob))]
impl harmos::Plugin for Process {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Resources;
    fn prepare(root: String) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Resources {
                root: root.into(),
                probes: AtomicU64::new(0),
            }),
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.mark("started")
    }
    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operations::<Implemented, _>(self.resources())
    }
    async fn stop(&mut self, context: &Supervisor) -> Result<(), Refusal> {
        if self.resources.root.join("shutdown-publication").exists() {
            context
                .publisher::<Process>()
                .publish_on::<Blob>(&Blob {
                    id: 47,
                    data: b"shutdown".to_vec(),
                })
                .await
                .map_err(Refusal::from)?;
            self.resources.mark("shutdown-publication-acknowledged")?;
        }
        self.resources.mark("shutdown")
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.mark("cleaned")
    }
}

impl Operation for Echoing {
    type Declaration = Echo;
    type Resources = Resources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _context: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        let constructed: Result<Self, Refusal> = async move {
            Ok(Self {
                resources,
                id: None,
                run_started: false,
            })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Refusal> = async {
            let operation = &mut operation;
            operation.resources.mark("echo-initialized")
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Blob,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Blob>, Refusal> {
        let result: ::core::result::Result<Blob, Self::Error> = async move {
            self.id = Some(request.id);
            self.resources
                .mark(&format!("echo-prepared-{}", request.id))?;
            self.resources
                .mark(&format!("echo-started-{}", request.id))?;
            let _work = Work {
                resources: &self.resources,
                id: request.id,
            };
            self.run_started = true;
            if request.id == 0 {
                std::future::pending::<()>().await;
            }
            if request.id == u64::MAX {
                std::process::exit(17);
            }
            Ok(request.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if self.id.is_none() {
            return self.resources.mark("echo-finalized-unstarted");
        }
        if self.run_started
            && !self
                .resources
                .root
                .join(format!(
                    "echo-dropped-{}",
                    self.id.expect("started request")
                ))
                .exists()
        {
            return Err(Refusal::of("operation work is still owned"));
        }
        self.resources.mark(&format!(
            "echo-finalized-{}",
            self.id.expect("started request")
        ))
    }
}

impl Operation for Probing {
    type Declaration = Probe;
    type Resources = Resources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        let sequence = resources.probes.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Self {
            _resources: resources,

            sequence,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Blob,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Blob>, Refusal> {
        let result: ::core::result::Result<Blob, Self::Error> = async move {
            // Report Construction Count Without Filesystem Effects In The Constructor.
            if request.id == u64::MAX {
                return Ok(Blob {
                    id: self.sequence,
                    data: Vec::new(),
                });
            }
            Ok(request.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Publishing {
    type Declaration = Nested;
    type Resources = Resources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _context: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        let constructed: Result<Self, Refusal> = async move {
            Ok(Self {
                resources,
                id: None,
            })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Refusal> = async {
            let operation = &mut operation;
            operation.resources.mark("nested-initialized")
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Blob,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Blob>, Refusal> {
        let result: ::core::result::Result<Blob, Self::Error> = async move {
            self.id = Some(request.id);
            self.resources
                .mark(&format!("nested-prepared-{}", request.id))?;
            host.publisher::<Process>()
                .publish_on::<Blob>(&*request)
                .await
                .map_err(Refusal::from)?;
            Ok(request.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if self.id.is_none() {
            return self.resources.mark("nested-finalized-unstarted");
        }
        self.resources.mark(&format!(
            "nested-finalized-{}",
            self.id.expect("started request")
        ))
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
