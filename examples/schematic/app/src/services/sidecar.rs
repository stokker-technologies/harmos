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
use std::time::Duration;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use crate::app::{Runtime, SchematicEditor};
use crate::lanes::Lanes;
use harmos::plugin::sidecar::{Executable, Managed, ManagedError, Notice, Supervised};
use harmos::plugin::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos::services::{Resident, ServiceDefinition, ServiceLease};
use harmos::{Error, Result, services::Fault, services::Service, services::ServicePolicy};
use schematic_contract::native::Operations;

/* === Constants === */
const NAME: &str = "schematic-tools";

/* === Macros === */

/* === Type Aliases === */
type Driver = Managed<SchematicEditor, Executable, fn(&Service<SchematicEditor>, Notice), Lanes>;

/* === Structs === */
struct Provider {
    driver: std::result::Result<Driver, Arc<ManagedError>>,
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

/* === Public Functions === */
pub async fn start(
    runtime: &Runtime,
    path: &Path,
    directory: &Path,
) -> Result<ServiceLease<SchematicEditor>> {
    let program = std::fs::canonicalize(path).map_err(failed)?;
    let bytes = std::fs::read(&program).map_err(failed)?;
    let executable = Executable::at(&program)
        .map_err(failed)?
        .directory(directory);
    let declaration = executable.declaration();
    if declaration.id != NAME {
        return Err(Error::NotReady {
            domain: "schematic plugin identity",
        });
    }
    let definition = PluginDefinition {
        id: declaration.id.clone(),
        version: declaration.version.clone(),
        artifact: PluginArtifact::Sidecar {
            path: program,
            sha256: Sha256::digest(&bytes).into(),
            arguments: Vec::new(),
            directory: Some(directory.to_path_buf()),
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
    let service = ServiceDefinition {
        id: NAME.into(),
        policy: ServicePolicy::default(),
    };
    runtime
        .services
        .register(service, move |_: Arc<()>| Provider {
            driver: Managed::new_executable_catalog::<Operations>(
                original.clone(),
                definition.clone(),
                Lanes::Open,
                NonZeroUsize::new(4).unwrap(),
                NonZeroUsize::new(4096).unwrap(),
            )
            .map_err(|error| Arc::new(ManagedError::Lifecycle(error))),
        })?;
    let scope = runtime.services.root_scope();
    let provider = runtime.services.activate(NAME, &scope).await?;
    let ready = tokio::time::timeout(Duration::from_secs(5), provider.ready())
        .await
        .map_err(failed)
        .and_then(|result| result);
    if let Err(error) = ready {
        provider.release().await?;
        return Err(error);
    }
    Ok(provider)
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
