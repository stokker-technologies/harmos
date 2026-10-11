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
use std::{fs, io::Write, path::PathBuf, time::Duration};
// Third Party Imports

// Local Imports
use harmos::sidecar::{Refusal, RegistrationError};
use harmos_sidecar::{Registrar, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Program {
    resources: std::sync::Arc<()>,
    task: Option<tokio::task::JoinHandle<()>>,
    failure: Option<std::sync::Arc<tokio::task::JoinError>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Program {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: harmos::RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: harmos::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Witnesses Registration And Initialization Separately.",
    id = "admission-fixture",
    provides = (),
    requires = (),
    restart(
        credit = 3,
        sustained_health_ms = 250,
        backoff_base_ms = 10,
        backoff_cap_ms = 5_000
    )
)]
impl harmos::Plugin for Program {
    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        let catalog = fs::read_to_string(root().join("catalog")).unwrap();
        registrar.register("fixture", 1, &catalog)?;
        registrar.register("unknown", 2, &vec![3, 4])?;
        witness("registered");
        Ok(())
    }

    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;
    type Resources = ();

    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self {
            resources: std::sync::Arc::new(()),
            task: None,
            failure: None,
        })
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        witness("initialized");
        if root().join("refuse").exists() {
            return Err(Refusal::of("initialization refused"));
        }
        while !root().join("release").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        self.task = Some(tokio::spawn(async {
            loop {
                if root().join("crash").exists() {
                    std::process::exit(1);
                }
                if root().join("panic").exists() {
                    eprintln!("panic at host");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }));
        Ok(())
    }

    fn resources(&self) -> std::sync::Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos::operations::WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn shutdown(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if let Some(failure) = &self.failure {
            return Err(Refusal::of(failure));
        }
        if let Some(task) = &mut self.task {
            task.abort();
            let joined = task.await;
            self.task = None;
            if let Err(error) = joined
                && !error.is_cancelled()
            {
                let error = std::sync::Arc::new(error);
                let refusal = Refusal::of(&error);
                self.failure = Some(error);
                return Err(refusal);
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("the admission fixture runtime builds");
    runtime
        .block_on(harmos::sidecar::run_plugin::<Program>(
            std::num::NonZeroUsize::new(1).unwrap(),
        ))
        .expect("the admission fixture serves its declared catalog");
}

fn root() -> PathBuf {
    std::env::var_os("HARMOS_TEST_ADMISSION").unwrap().into()
}

fn witness(name: &str) {
    writeln!(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root().join(name))
            .unwrap(),
        "called"
    )
    .unwrap();
}

/* === Tests === */
