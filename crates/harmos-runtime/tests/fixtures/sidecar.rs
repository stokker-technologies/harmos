/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod linked {
    #[harmos_runtime::stream(id = "foreign", window = 4)]
    #[harmos_runtime::message]
    #[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct Foreign {
        pub value: u32,
    }

    const POLICY: harmos_runtime::operations::OperationPolicy = harmos_runtime::operations::OperationPolicy {
        timeout: std::time::Duration::from_millis(100),
        attempt_timeout: std::time::Duration::from_millis(100),
        retries: harmos_runtime::operations::RetryPolicy::Never,
        restart: harmos_runtime::operations::RestartPolicy::Never,
    };

    #[harmos_runtime::message]
    #[harmos_runtime::operation(id = "guest.echo", version = 1, policy = POLICY, response = String)]
    pub struct Echo {
        pub text: String,
    }

    #[harmos_runtime::catalog]
    pub enum Operations {
        Echo(Echo),
    }

    pub struct LinkedGuest {
        resources: std::sync::Arc<()>,
    }
    impl harmos_runtime::operations::lifecycle::Policy for LinkedGuest {
        const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy =
            harmos_runtime::operations::OperationPolicy {
                timeout: std::time::Duration::from_secs(2),
                attempt_timeout: std::time::Duration::from_secs(2),
                retries: harmos_runtime::operations::RetryPolicy::Never,
                restart: harmos_runtime::operations::RestartPolicy::Never,
            };
        const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
    }
    #[harmos_runtime::plugin(id = "meter", provides = Operations, requires = (), streams(Foreign))]
    impl harmos_runtime::Plugin for LinkedGuest {
        type Config = String;
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::guest::Context;
        type Resources = ();
        fn prepare(_: String) -> Result<Self, Self::Error> {
            Ok(Self {
                resources: std::sync::Arc::new(()),
            })
        }
        fn resources(&self) -> std::sync::Arc<()> {
            self.resources.clone()
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(
            &self,
            _: &mut harmos_runtime::operations::WireTable<Self::Context>,
        ) -> Result<(), harmos_runtime::operations::ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::{
    Operation, OperationPolicy, RestartPolicy, RetryPolicy, WireTable,
};
use harmos_runtime::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct Config {}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.ask",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_millis(25),
        attempt_timeout: Duration::from_millis(25),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = GetAskResponse,
    description = "An operation that never answers, so the host's deadline ends the call.",
)]
#[derive(Clone, PartialEq)]
struct Ask {}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
struct GetAskResponse {}

#[harmos_runtime::stream(
    description = "One row the fixture contributes alongside its identity.\nA stream id is a Python identifier, not an artifact id, so this one carries the `_` that the two subsets disagree on. The sidecar explicitly composes this row; its co-linked guest grants a different row from the same file.",
    id = "meter_reading",
    window = 8
)]
#[harmos_runtime::message]
#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MeterReading {
    pub millivolts: i32,
}

struct Stubborn {
    resources: Arc<Handler>,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.echo",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_millis(80),
        attempt_timeout: Duration::from_millis(80),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = String,
    description = "Answers with what it was handed, publishing first when asked to.",
)]
struct Echo {
    text: String,
}

struct Handler;

struct Asking {
    _resources: Arc<Handler>,
}

struct Echoing {
    _resources: Arc<Handler>,
}

/* === Enums === */
#[harmos_runtime::catalog]
enum Operations {
    Ask(Ask),
    Echo(Echo),
}

/* === Traits === */

/* === Implementations === */
impl harmos_runtime::operations::lifecycle::Policy for Stubborn {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

#[harmos_runtime::plugin(description = "Exercises native sidecar supervision.",
    id = "meter",
    streams(MeterReading),
    provides = Operations,
    requires(harmos_runtime::operations::streams::Publish),
    restart(
        credit = 3,
        sustained_health_ms = 250,
        backoff_base_ms = 10,
        backoff_cap_ms = 5_000
    )
)]
impl harmos_runtime::Plugin for Stubborn {
    type Context = Supervisor;
    type Config = Config;
    type Error = Refusal;
    type Resources = Handler;

    fn prepare(_: Config) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Handler),
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        eprintln!("the meter is warming up");
        Ok(())
    }
    fn resources(&self) -> Arc<Handler> {
        Arc::clone(&self.resources)
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Asking>(self.resources())?;
        table.bind_operation::<Echoing>(self.resources())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        if std::env::var("HARMOS_TEST_DELAY_SHUTDOWN").as_deref() == Ok("1") {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if std::env::var("HARMOS_TEST_IGNORE_CLEANUP").as_deref() == Ok("1") {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        Ok(())
    }
}

impl Operation for Asking {
    type Declaration = Ask;
    type Resources = Handler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Handler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut Ask,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<GetAskResponse>, Refusal> {
        let result: ::core::result::Result<GetAskResponse, Self::Error> =
            async move { std::future::pending().await }.await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Echoing {
    type Declaration = Echo;
    type Resources = Handler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Handler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Echo,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            if request.text == "publish-foreign" {
                // Reach The Host With A Selected Core Leaf And An Ungranted Stream.
                host.call(harmos_runtime::operations::streams::Publish {
                    stream: <linked::Foreign as harmos_runtime::StreamSpec>::ID.to_owned(),
                    schema: <linked::Foreign as harmos_runtime::StreamSpec>::VERSION,
                    sequence: Some(1),
                    published_unix_micros: None,
                    context: None,
                    payload: harmos_runtime::Message::encode_to_vec(&linked::Foreign { value: 1 }),
                    following: Vec::new(),
                })
                .await
                .map_err(Refusal::from)?;
            }
            Ok(request.text.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    // The co-linked guest's declaration is inert data no code in this native
    // executable reads, and `#[used]` reaches the compiler rather than the
    // linker: an MSVC link dropped all three of its fragments, leaving the
    // executable inspectable as a sidecar and not as the guest it also
    // declares. `harmos_runtime::guest::export_plugin!` roots them from a component's first
    // export the same way `harmos_runtime::sidecar::run_plugin` roots the native ones; a
    // native binary reaches no such export, so it states the reference here.
    std::hint::black_box(
        <linked::LinkedGuest as harmos_domain::plugin::Declared>::markers().unwrap(),
    );

    if std::env::var("HARMOS_TEST_SIDECAR").as_deref() == Ok("silent") {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("the fixture runtime builds");
    runtime
        .block_on(harmos_runtime::sidecar::run_plugin::<Stubborn>(
            NonZeroUsize::new(8).unwrap(),
        ))
        .expect("the fixture serves its gRPC boundary");
}

/* === Tests === */
