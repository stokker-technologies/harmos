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
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use crate::contract::{Exchange, Operations, PAYLOAD_BYTES, Payload};
use harmos::plugin::sidecar::{Executable, Managed, ManagedError, Notice, Supervised};
use harmos::plugin::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos::services::{Resident, ServiceDefinition, ServiceLease};
use harmos::{Error, Result, services::Fault, services::Service, services::ServicePolicy};
use schematic_app::app::{Runtime, SchematicEditor};
use schematic_app::lanes::Lanes;

/* === Constants === */
const NAME: &str = "lifecycle-worker";
const TRAFFIC: &str = "schematic-sidecar-traffic";

/* === Macros === */

/* === Type Aliases === */
type Driver = Managed<SchematicEditor, Executable, fn(&Service<SchematicEditor>, Notice), Lanes>;

/* === Structs === */
struct Provider {
    driver: std::result::Result<Driver, Arc<ManagedError>>,
}

struct Traffic {
    completed: Arc<AtomicU64>,
}

pub struct Workload {
    provider: ServiceLease<SchematicEditor>,
    traffic: ServiceLease<SchematicEditor>,
    completed: Arc<AtomicU64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Resident<SchematicEditor> for Provider {
    type Error = Arc<ManagedError>;
    async fn attempt(
        &mut self,
        context: Service<SchematicEditor>,
    ) -> std::result::Result<(), Fault<Self::Error>> {
        let driver = self
            .driver
            .as_mut()
            .map_err(|error| Fault::Application(Arc::clone(error)))?;
        driver.attempt(context).await.map_err(|error| match error {
            Fault::Environmental(error) => Fault::Environmental(Arc::new(error)),
            Fault::Application(error) => Fault::Application(Arc::new(error)),
        })
    }
    async fn stop(&mut self) -> std::result::Result<(), Self::Error> {
        match &mut self.driver {
            Ok(driver) => driver.stop().await.map_err(Arc::new),
            Err(_) => Ok(()),
        }
    }
    async fn finalize(&mut self) -> std::result::Result<(), Self::Error> {
        match &mut self.driver {
            Ok(driver) => driver.finalize().await.map_err(Arc::new),
            Err(_) => Ok(()),
        }
    }
}

impl Resident<SchematicEditor> for Traffic {
    type Error = Error;
    async fn attempt(
        &mut self,
        mut context: Service<SchematicEditor>,
    ) -> std::result::Result<(), Fault<Error>> {
        let operations = context.operations().clone();
        loop {
            let sequence = self.completed.load(Ordering::SeqCst);
            let request = Payload {
                sequence,
                bytes: vec![sequence as u8; PAYLOAD_BYTES],
            };
            let Some(response) = context
                .until_stopping(operations.call::<Exchange>(request))
                .await
            else {
                return Ok(());
            };
            let response = match response {
                Ok(response) => response,
                // Wait For Resident Settlement When Admission Has Closed
                Err(Error::Closing { .. }) => {
                    context.until_stopping(std::future::pending::<()>()).await;
                    return Ok(());
                }
                Err(error) => return Err(Fault::Application(error)),
            };
            if response.sequence != sequence
                || response.bytes.len() != PAYLOAD_BYTES
                || response.bytes.iter().any(|byte| *byte != sequence as u8)
            {
                return Err(Fault::Application(Error::NotReady {
                    domain: "worker payload validation",
                }));
            }
            self.completed.fetch_add(1, Ordering::SeqCst);
            context.ready();
        }
    }
    async fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Workload {
    pub async fn verify(&self) -> Result<u64> {
        self.traffic.instance().join().await?;
        self.provider.instance().join().await?;
        let completed = self.completed.load(Ordering::SeqCst);
        if completed == 0 {
            return Err(Error::NotReady {
                domain: "worker traffic",
            });
        }
        Ok(completed)
    }
}

/* === Public Functions === */
pub async fn start(runtime: &Runtime, path: &Path) -> Result<Workload> {
    let program = std::fs::canonicalize(path).map_err(failed)?;
    let bytes = std::fs::read(&program).map_err(failed)?;
    let executable = Executable::at(&program).map_err(failed)?;
    let declaration = executable.declaration();
    if declaration.id != NAME {
        return Err(Error::NotReady {
            domain: "worker executable identity",
        });
    }
    let definition = PluginDefinition {
        id: declaration.id.clone(),
        version: declaration.version.clone(),
        artifact: PluginArtifact::Sidecar {
            path: program,
            sha256: Sha256::digest(&bytes).into(),
            arguments: Vec::new(),
            directory: None,
            environment: Default::default(),
        },
        configuration: b"null".to_vec(),
        requires: Vec::new(),
        contributions: PluginContributions {
            lifecycle: Some(declaration.contracts.lifecycle.clone()),
            operations: declaration.operations.clone(),
            wire_contracts: declaration.contracts.provides.clone(),
            required_contracts: Some(declaration.contracts.requires.clone()),
            sections: declaration.sections.clone(),
            streams: declaration.streams.clone(),
            observers: declaration.observers.clone(),
            observer_controls: declaration.observer_controls().cloned(),
            listeners: declaration.listeners.clone(),
            ..PluginContributions::default()
        },
    };
    let original: Supervised<SchematicEditor, _, fn(&Service<SchematicEditor>, Notice)> =
        Supervised::new(executable, diagnostics);
    runtime.services.register(
        ServiceDefinition {
            id: NAME.into(),
            policy: ServicePolicy::default(),
        },
        move |_: Arc<()>| Provider {
            driver: Managed::new_executable_catalog::<Operations>(
                original.clone(),
                definition.clone(),
                Lanes::Open,
                NonZeroUsize::new(4).unwrap(),
                NonZeroUsize::new(4096).unwrap(),
            )
            .map_err(|error| Arc::new(ManagedError::Lifecycle(error))),
        },
    )?;
    let provider = runtime
        .services
        .activate(NAME, &runtime.services.root_scope())
        .await?;
    tokio::time::timeout(Duration::from_secs(5), provider.ready())
        .await
        .map_err(failed)??;
    let completed = Arc::new(AtomicU64::new(0));
    let counts = Arc::clone(&completed);
    runtime.services.register(
        ServiceDefinition {
            id: TRAFFIC.into(),
            policy: ServicePolicy::default(),
        },
        move |_: Arc<()>| Traffic {
            completed: Arc::clone(&counts),
        },
    )?;
    let traffic = runtime
        .services
        .activate(TRAFFIC, &runtime.services.root_scope())
        .await?;
    tokio::time::timeout(Duration::from_secs(5), traffic.ready())
        .await
        .map_err(failed)??;
    Ok(Workload {
        provider,
        traffic,
        completed,
    })
}

/* === Private Functions === */
fn diagnostics(_: &Service<SchematicEditor>, notice: Notice) {
    if let Notice::Logged { line } = notice {
        eprintln!("{NAME}: {line}");
    }
}

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
