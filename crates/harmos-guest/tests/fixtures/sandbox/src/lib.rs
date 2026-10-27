/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Guest For Every Sandbox Behaviour The Host Stories Drive: The Configuration
// String Selects A Lifecycle Mode And Request Fields Select Handler Behaviour

/* === Modules === */

pub mod contract;

/* === Re-Exports === */

/* === Imports === */

use harmos_guest::builtins::{lifecycle, streams};

// Standard Library Imports
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports

// Local Imports
use contract::{Echoed, ProviderEchoOperation};
use harmos_domain::operations::{OperationPolicy, RetryPolicy};
use harmos_guest::kit::Context;
use harmos_guest::operations::{ServingError, WireTable};
use harmos_guest::{Host, PublishError};

/* === Constants === */

const LIFECYCLE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
};

/* === Macros === */

harmos_guest::export_plugin!(Sandbox, capacity = 2);

/* === Type Aliases === */

/* === Structs === */

pub struct Sandbox {
    config: String,
    check_publication_revocation: bool,
    check_dependency_revocation: bool,
    owner: Arc<Owner>,
}

#[derive(Default)]
pub struct Owner {
    initialized_context: Mutex<Option<Context>>,
    attempts: Mutex<std::collections::HashMap<String, usize>>,
    saved_attempts: Mutex<std::collections::HashMap<String, Context>>,
    ran: AtomicUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl lifecycle::Policy for Sandbox {
    const INITIALIZATION_POLICY: OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: OperationPolicy = LIFECYCLE;
}

#[harmos_guest::plugin(
    id = "sandbox_guest",
    description = "Answers the host harness's sandbox probes.",
    provides = contract::Operations,
    requires(contract::ProviderEchoOperation, streams::Publish),
    streams(Echoed)
)]
impl harmos_guest::Plugin for Sandbox {
    type Config = String;
    type Error = std::io::Error;
    type Context = Context;
    type Resources = Owner;

    fn register(registrar: &mut harmos_guest::Registrar) -> Result<(), harmos_guest::RegistrationError> {
        registrar.register("zeta.catalog", 2, &"second")?;
        registrar.register("alpha.catalog", 1, &"first")
    }

    fn prepare(config: String) -> Result<Self, Self::Error> {
        Ok(Self {
            config,
            check_publication_revocation: false,
            check_dependency_revocation: false,
            owner: Arc::new(Owner::default()),
        })
    }

    fn resources(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }

    // A Mismatched Guest Declares Its Catalog But Binds None Of It
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), ServingError> {
        if self.config == "mismatch" {
            return Ok(());
        }
        let mut dependencies = harmos_guest::Dependencies::new();
        if self.config != "missing-resource" {
            dependencies.insert(self.resources()).map_err(|_| ServingError::Catalog)?;
        }
        let entries = contract::Operations::registration(dependencies);
        table.register(entries)
    }

    async fn initialize(&mut self, context: &Context) -> Result<(), Self::Error> {
        *self.owner.initialized_context.lock().map_err(|error| std::io::Error::other(error.to_string()))? = Some(context.clone());

        // Publication Modes
        if self.config == "publications" || self.config == "failed-publications" {
            progress(context, "initialize").await.map_err(std::io::Error::other)?;
            if self.config == "failed-publications" {
                return Err(std::io::Error::other("initialization refused after publication"));
            }
            self.check_publication_revocation = true;
        }

        // Submitted Children Through The Provider Dependency
        if self.config == "submit" {
            return submitted(context).await;
        }

        // Dependency Mode
        if self.config == "dependencies" {
            context
                .call::<contract::ProviderEchoOperation>(ProviderEchoOperation { say: "initialize".into() })
                .await
                .map_err(std::io::Error::other)?;
            self.check_dependency_revocation = true;
        }
        Ok(())
    }

    async fn stop(&mut self, _: &Context) -> Result<(), Self::Error> {
        if self.config == "shutdown-pending" {
            std::future::pending::<()>().await;
        }
        let context = self
            .owner
            .initialized_context
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .clone()
            .ok_or_else(|| std::io::Error::other("the initialization context is gone"))?;
        if self.check_publication_revocation {
            let result = progress(&context, "cleanup").await;
            if !matches!(result, Err(PublishError::Call(_))) {
                return Err(std::io::Error::other("retirement must refuse a new publication"));
            }
        }
        if self.check_dependency_revocation {
            let result = context.call::<contract::ProviderEchoOperation>(ProviderEchoOperation { say: "cleanup".into() }).await;
            if result.is_ok() {
                return Err(std::io::Error::other("retirement must refuse a new dependency call"));
            }
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.owner.initialized_context.lock().map_err(|error| std::io::Error::other(error.to_string()))?.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn progress(context: &Context, value: &str) -> Result<(), PublishError> {
    context.publisher::<Sandbox>().publish_on::<Echoed>(&Echoed { said: value.into() }).await
}

// Drives Every Child Control Once: Status And Join, Cancel, Release By Drop, Detach
async fn submitted(context: &Context) -> Result<(), std::io::Error> {
    use harmos_guest::operations::Instance;
    use harmos_guest::operations::instance::Phase;
    let request = |say: &str| ProviderEchoOperation { say: say.into() };

    let joined = context.submit::<contract::ProviderEchoOperation>(request("joined")).await.map_err(std::io::Error::other)?;
    let status = joined.status().await.map_err(std::io::Error::other)?;
    if status.phase != Phase::Running {
        return Err(std::io::Error::other(format!("a live child reported {:?}", status.phase)));
    }
    let answer = joined.join().await.map_err(std::io::Error::other)?;
    if answer.said != "joined" {
        return Err(std::io::Error::other(format!("joined {}", answer.said)));
    }

    let cancelled = context.submit::<contract::ProviderEchoOperation>(request("cancelled")).await.map_err(std::io::Error::other)?;
    if Instance::cancel_and_join(cancelled).await.is_ok() {
        return Err(std::io::Error::other("a cancelled child must not answer"));
    }

    let dropped = context.submit::<contract::ProviderEchoOperation>(request("dropped")).await.map_err(std::io::Error::other)?;
    drop(dropped);

    let detached = context.submit::<contract::ProviderEchoOperation>(request("detached")).await.map_err(std::io::Error::other)?;
    Instance::detach(detached);
    Ok(())
}

/* === Tests === */
