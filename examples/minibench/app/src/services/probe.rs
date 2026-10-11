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
use std::time::Duration;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use crate::app::{Bench, Lanes, Runtime};
use crate::config::Config;
use crate::error::ProbeError;
use crate::streams::Reading;
use harmos::load::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos::services::{ServiceDefinition, ServiceLease};
use harmos::sidecar::inventory::{Declaration, Facility};
use harmos::sidecar::{Executable, Managed, ManagedError, Notice, Supervised};
use harmos::{Fault, Service, ServicePolicy};
use minibench_contract::probe::{Operations, Settings};

/* === Constants === */
pub const NAME: &str = "probe-bridge";
const CAPACITY: NonZeroUsize = NonZeroUsize::new(64).unwrap();
const ENCODING_MEMORY_BYTES: NonZeroUsize = NonZeroUsize::new(4096).unwrap();

/* === Macros === */

/* === Type Aliases === */
pub type Probe = Supervised<Bench, Executable, fn(&Service<Bench>, Notice)>;
type Driver = Managed<Bench, Executable, fn(&Service<Bench>, Notice), Lanes>;

/* === Structs === */
pub struct PublishedReading;

pub struct Attachment {
    pub sidecar: Probe,
    pub supervision: ServicePolicy,
    pub definition: PluginDefinition,
}

struct Resident {
    driver: Result<Driver, Arc<ManagedError>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::StreamSpec for PublishedReading {
    type Row = Reading;
    const ID: &'static str = minibench_contract::probe::READING.0;
    const VERSION: u32 = minibench_contract::probe::READING.1;
    const WINDOW: usize = Reading::WINDOW;
}

impl harmos::services::Resident<Bench> for Resident {
    type Error = Arc<ManagedError>;

    async fn attempt(&mut self, context: Service<Bench>) -> Result<(), Fault<Self::Error>> {
        let driver = match &mut self.driver {
            Ok(driver) => driver,
            Err(source) => return Err(Fault::Application(Arc::clone(source))),
        };
        harmos::services::Resident::attempt(driver, context)
            .await
            .map_err(|fault| match fault {
                Fault::Environmental(source) => Fault::Environmental(Arc::new(source)),
                Fault::Application(source) => Fault::Application(Arc::new(source)),
            })
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        match &mut self.driver {
            Ok(driver) => harmos::services::Resident::stop(driver)
                .await
                .map_err(Arc::new),
            Err(_) => Ok(()),
        }
    }

    async fn finalize(&mut self) -> Result<(), Self::Error> {
        match &mut self.driver {
            Ok(driver) => harmos::services::Resident::finalize(driver)
                .await
                .map_err(Arc::new),
            Err(_) => Ok(()),
        }
    }
}

/* === Public Functions === */
pub fn inspect(config: &Config) -> Result<Declaration, ProbeError> {
    Ok(Declaration::inspect(
        &std::fs::read(config.bridge())?,
        Facility::Sidecar,
    )?)
}

pub fn attach(config: &Config) -> Result<Attachment, ProbeError> {
    let program = std::fs::canonicalize(config.bridge())?;
    let bytes = std::fs::read(&program)?;
    let declaration = Declaration::inspect(&bytes, Facility::Sidecar)?;
    if declaration.id != NAME {
        return Err(std::io::Error::other("the probe executable has a different owner").into());
    }
    let executable = Executable::at(&program)?;
    if executable.declaration() != &declaration {
        return Err(std::io::Error::other("probe changed during inspection").into());
    }
    let supervision = ServicePolicy {
        credit: 3,
        sustained_health: Duration::from_millis(30_000),
        backoff_base: Duration::from_millis(250),
        backoff_cap: Duration::from_millis(10_000),
        ..ServicePolicy::default()
    };
    let observer_controls = declaration.observer_controls().cloned();
    let definition = PluginDefinition {
        id: declaration.id.clone(),
        version: declaration.version,
        artifact: PluginArtifact::Sidecar {
            path: program,
            sha256: Sha256::digest(&bytes).into(),
            arguments: Vec::new(),
            directory: None,
            environment: Default::default(),
        },
        configuration: harmos::artifact::encode_config(&Settings::new(config.device()))?,
        requires: Vec::new(),
        contributions: PluginContributions {
            lifecycle: Some(declaration.contracts.lifecycle),
            operations: declaration.operations,
            wire_contracts: declaration.contracts.provides,
            required_contracts: Some(declaration.contracts.requires),
            observer_controls,
            sections: declaration.sections,
            streams: declaration.streams,
            observers: declaration.observers,
            listeners: declaration.listeners,
            ..PluginContributions::default()
        },
    };
    let sidecar: Probe = Supervised::new(executable, log);

    Ok(Attachment {
        sidecar: sidecar.ingest::<PublishedReading>(),
        supervision,
        definition,
    })
}

pub async fn activate(
    runtime: &Runtime,
    attachment: Attachment,
) -> harmos::Result<ServiceLease<Bench>> {
    runtime
        .operations
        .register_wire_operation::<harmos::sidecar::Publish<_, _, _>>(
            "probe-host",
            Lanes::Open,
            Arc::new(attachment.sidecar.clone()),
        )?;
    let sidecar = attachment.sidecar;
    let original = sidecar.clone();
    let definition = attachment.definition;
    runtime.services.register(
        ServiceDefinition {
            id: NAME.into(),
            policy: attachment.supervision,
        },
        move |_: Arc<()>| Resident {
            driver: Managed::new_executable_catalog::<Operations>(
                original.clone(),
                definition.clone(),
                Lanes::Open,
                CAPACITY,
                ENCODING_MEMORY_BYTES,
            )
            .map_err(|source| Arc::new(ManagedError::Lifecycle(source))),
        },
    )?;
    let lease = runtime
        .services
        .activate(NAME, &runtime.services.root_scope())
        .await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        lease.ready().await?;
        sidecar
            .operations()
            .map_err(|source| harmos::Error::OperationFailed {
                operation: NAME.into(),
                source: Arc::new(source),
            })?;
        Ok::<(), harmos::Error>(())
    })
    .await
    .map_err(|source| harmos::Error::TaskFailed {
        domain: "probe readiness",
        source: Arc::new(source),
    })??;
    Ok(lease)
}

/* === Private Functions === */
fn log(_service: &Service<Bench>, notice: Notice) {
    if let Notice::Logged { line } = notice {
        eprintln!("{NAME}: {line}");
    }
}

/* === Tests === */
