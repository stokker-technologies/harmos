/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Guest For Every Sandbox Behaviour The Host Stories Drive: The Configuration
// String Selects A Lifecycle Mode And Request Fields Select Operation Behaviour

/* === Modules === */

pub mod contract;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports

// Local Imports
use contract::{Counts, Decline, Echo, Echoed, Fail, ProviderEcho, Read, Spin, Stage, Value, Wait};
use harmos_domain::operations::{FailureKind, OperationPolicy, RestartPolicy, RetryPolicy, lifecycle, streams};
use harmos_guest::operations::{ServingError, WireTable};
use harmos_guest::{Context, Host, OperationRefusal, PublishError};

/* === Constants === */

const LIFECYCLE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
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
    fail_initialize: AtomicBool,
    child_initialize: AtomicBool,
    initialized: AtomicUsize,
    ran: AtomicUsize,
    stopped: AtomicUsize,
    finalized: AtomicUsize,
}

struct Echoing {
    owner: Arc<Owner>,
}

struct Declining;

struct Spinning;

struct Waiting;

struct Failing;

struct Reading {
    owner: Arc<Owner>,
    value: String,
    attempts: usize,
    cleanups: usize,
    saved: Option<Context>,
}

struct Counting {
    owner: Arc<Owner>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl lifecycle::Policy for Sandbox {
    const INITIALIZATION_POLICY: OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: OperationPolicy = LIFECYCLE;
}

#[harmos_guest::plugin(
    id = "sandbox-guest",
    description = "Answers the host harness's sandbox probes.",
    provides = contract::Operations,
    requires(ProviderEcho, streams::Publish),
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
        let owner = Owner {
            fail_initialize: AtomicBool::new(config == "initialize-fail"),
            child_initialize: AtomicBool::new(config == "initialize-child"),
            ..Owner::default()
        };
        Ok(Self { config, check_publication_revocation: false, check_dependency_revocation: false, owner: Arc::new(owner) })
    }

    fn resources(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }

    // A Mismatched Guest Declares Its Catalog But Binds None Of It
    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
        if self.config == "mismatch" {
            return Ok(());
        }
        table.bind_operation::<Echoing>(self.resources())?;
        table.bind_operation::<Declining>(self.resources())?;
        table.bind_operation::<Spinning>(self.resources())?;
        table.bind_operation::<Waiting>(self.resources())?;
        table.bind_operation::<Failing>(self.resources())?;
        table.bind_operation::<Reading>(self.resources())?;
        table.bind_operation::<Counting>(self.resources())
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
            context.call(ProviderEcho { say: "initialize".into() }).await.map_err(std::io::Error::other)?;
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
            let result = context.call(ProviderEcho { say: "cleanup".into() }).await;
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

#[harmos_guest::operation]
impl Echoing {
    async fn initialize(owner: Arc<Owner>, _: &Context) -> Self {
        Self { owner }
    }

    async fn run(&mut self, context: &Context, request: &mut Echo) -> Result<Echoed, OperationRefusal> {
        // Calls Through The Saved Initialization Context
        if let Some(value) = request.say.strip_prefix("saved-publish:") {
            let saved = self.saved()?;
            progress(&saved, value).await.map_err(OperationRefusal::of)?;
            return Ok(Echoed { said: format!("published {value}") });
        }
        if let Some(value) = request.say.strip_prefix("saved-dependency:") {
            let saved = self.saved()?;
            return saved.call(ProviderEcho { say: value.into() }).await.map(harmos_guest::operations::Response::into_owned);
        }

        // Calls Through The Current Context
        if let Some(say) = request.say.strip_prefix("dependency:") {
            return context.call(ProviderEcho { say: say.to_owned() }).await.map(harmos_guest::operations::Response::into_owned);
        }
        if let Some(value) = request.say.strip_prefix("publish:") {
            progress(context, value).await.map_err(OperationRefusal::from)?;
        }
        if request.say == "publish-many" {
            for index in 0..8 {
                progress(context, &index.to_string()).await.map_err(OperationRefusal::from)?;
            }
        }

        // Memory The Sandbox Must Refuse
        if request.say == "grow" {
            std::hint::black_box(vec![1u8; 128 * 1024 * 1024]);
        }
        Ok(Echoed { said: format!("heard {}", request.say) })
    }
}

impl Echoing {
    fn saved(&self) -> Result<Context, OperationRefusal> {
        self.owner
            .initialized_context
            .lock()
            .map_err(OperationRefusal::of)?
            .clone()
            .ok_or_else(|| OperationRefusal::of("SDK initialization context is absent"))
    }
}

#[harmos_guest::operation]
impl Declining {
    async fn initialize(_: Arc<Owner>, _: &Context) -> Self {
        Self
    }

    async fn run(&mut self, _: &Context, _: &mut Decline) -> Result<Echoed, OperationRefusal> {
        Err(OperationRefusal::new(7, "the probe declines"))
    }
}

#[harmos_guest::operation]
impl Spinning {
    async fn initialize(_: Arc<Owner>, _: &Context) -> Self {
        Self
    }

    async fn run(&mut self, _: &Context, _: &mut Spin) -> Result<Echoed, OperationRefusal> {
        let mut counted = 0u64;
        loop {
            counted = counted.wrapping_add(1);
            std::hint::black_box(counted);
        }
    }
}

#[harmos_guest::operation]
impl Waiting {
    async fn initialize(_: Arc<Owner>, _: &Context) -> Self {
        Self
    }

    // The Provider Answers Slowly; Nothing Here Burns Fuel While It Waits
    async fn run(&mut self, context: &Context, _: &mut Wait) -> Result<Echoed, OperationRefusal> {
        let _ = context.call(ProviderEcho { say: "slow".into() }).await;
        // Resuming Reaches The Epoch Check A Suspended Guest Could Not
        for step in 0..4096u32 {
            std::hint::black_box(step);
        }
        Ok(Echoed { said: "waited".to_owned() })
    }
}

#[harmos_guest::operation]
impl Failing {
    async fn initialize(_: Arc<Owner>, _: &Context) -> Self {
        Self
    }

    async fn run(&mut self, _: &Context, _: &mut Fail) -> Result<Echoed, OperationRefusal> {
        panic!("requested guest trap")
    }
}

#[harmos_guest::operation]
impl Reading {
    async fn initialize(owner: Arc<Owner>, context: &Context) -> Result<Self, std::io::Error> {
        owner.initialized.fetch_add(1, Ordering::SeqCst);
        if owner.fail_initialize.swap(false, Ordering::SeqCst) {
            return Err(std::io::Error::other("partial initialization failed"));
        }
        let mut saved = None;
        if owner.child_initialize.swap(false, Ordering::SeqCst) {
            let child = context.call(Read { value: "leaf".into() }).await.map_err(std::io::Error::other)?;
            assert_eq!(child.value, "leaf:1");
            saved = Some(context.clone());
        }
        Ok(Self { owner, value: String::new(), attempts: 0, cleanups: 0, saved })
    }

    async fn run(&mut self, context: &Context, value: &mut Read) -> Result<Value, std::io::Error> {
        self.value.clone_from(&value.value);
        self.owner.ran.fetch_add(1, Ordering::SeqCst);

        // Retired Initialization Context
        if self.value == "initialize-child" {
            let saved = self.saved.as_ref().expect("original initialization context");
            let retired = saved.call(Read { value: "leaf".into() }).await;
            if retired.is_ok() {
                return Err(std::io::Error::other("initialization context remained active"));
            }
        }

        // Context Values Overlaid For A Child
        if self.value == "context-leaf" {
            let stage = stage(context)?;
            return Ok(Value { value: stage.id });
        }
        if self.value == "context-parent" {
            let parent = stage(context)?;
            assert_eq!(parent.id, "parent");
            let caller = context.with_value(Stage { id: "child".into() }).map_err(std::io::Error::other)?;
            let child = caller.call(Read { value: "context-leaf".into() }).await.map_err(std::io::Error::other)?;
            let original = stage(context)?;
            return Ok(Value { value: format!("{}:{}", original.id, child.value) });
        }

        // Children And Retried Attempts
        self.attempts += 1;
        if self.value == "leaf" {
            return Ok(Value { value: "leaf:1".into() });
        }
        if self.value == "parent" {
            if let Some(saved) = &self.saved {
                let stale = saved.call(Read { value: "leaf".into() }).await;
                if stale.is_ok() {
                    return Err(std::io::Error::other("stale context was accepted"));
                }
            }
            let child = context.call(Read { value: "leaf".into() }).await.map_err(std::io::Error::other)?;
            if self.attempts > 1 {
                return Ok(Value { value: format!("parent:2:stale:{}", child.value) });
            }
            self.saved = Some(context.clone());
        }
        if self.attempts == 1 {
            return Err(std::io::Error::other("first transient attempt"));
        }
        Ok(Value { value: format!("{}:{}", self.value, self.attempts) })
    }

    async fn stop(&mut self) -> Result<(), std::io::Error> {
        self.owner.stopped.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), std::io::Error> {
        self.owner.finalized.fetch_add(1, Ordering::SeqCst);
        self.cleanups += 1;
        if self.value == "cleanup" && self.cleanups == 1 {
            return Err(std::io::Error::other("first cleanup retained"));
        }
        Ok(())
    }

    fn classify(_: &std::io::Error) -> FailureKind {
        FailureKind::Transient
    }
}

#[harmos_guest::operation]
impl Counting {
    async fn initialize(owner: Arc<Owner>, _: &Context) -> Self {
        Self { owner }
    }

    async fn run(&mut self, _: &Context, _: &mut Counts) -> Result<Value, std::io::Error> {
        let owner = &self.owner;
        Ok(Value {
            value: format!(
                "{}:{}:{}:{}",
                owner.initialized.load(Ordering::SeqCst),
                owner.ran.load(Ordering::SeqCst),
                owner.stopped.load(Ordering::SeqCst),
                owner.finalized.load(Ordering::SeqCst)
            ),
        })
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
    let request = |say: &str| ProviderEcho { say: say.into() };

    let joined = context.submit(request("joined")).await.map_err(std::io::Error::other)?;
    let status = joined.status().await.map_err(std::io::Error::other)?;
    if status.phase != Phase::Running {
        return Err(std::io::Error::other(format!("a live child reported {:?}", status.phase)));
    }
    let answer = joined.join().await.map_err(std::io::Error::other)?;
    if answer.said != "joined" {
        return Err(std::io::Error::other(format!("joined {}", answer.said)));
    }

    let cancelled = context.submit(request("cancelled")).await.map_err(std::io::Error::other)?;
    if Instance::cancel_and_join(cancelled).await.is_ok() {
        return Err(std::io::Error::other("a cancelled child must not answer"));
    }

    let dropped = context.submit(request("dropped")).await.map_err(std::io::Error::other)?;
    drop(dropped);

    let detached = context.submit(request("detached")).await.map_err(std::io::Error::other)?;
    Instance::detach(detached);
    Ok(())
}

fn stage(context: &Context) -> Result<Stage, std::io::Error> {
    context.context().value::<Stage>().map_err(std::io::Error::other)?.ok_or_else(|| std::io::Error::other("the caller set no stage"))
}

/* === Tests === */
