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
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::Runtime;
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Measure;

#[harmos_runtime::message]
#[harmos_runtime::operation(
    response = Measured,
    id = "clock.measure",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
)]
struct MeasureContract {
    wait_nanos: u64,
}

// The Scope's Elapsed Clock, Then The Wall Clock, Across One Wait.
#[harmos_runtime::message]
struct Measured {
    scoped_nanos: u64,
    wall_nanos: u64,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Operations {
    MeasureContract(MeasureContract),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Measure {
    type Declaration = MeasureContract;
    type Resources = ();
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_resources: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, wait: &mut MeasureContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Measured>, Self::Error> {
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let scope = context.scope();
            let scoped = scope.now();
            let wall = Instant::now();
            tokio::time::sleep(Duration::from_nanos(wait.wait_nanos)).await;
            Ok(Measured { scoped_nanos: nanos(scope.now() - scoped), wall_nanos: nanos(wall.elapsed()) })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).expect("measured waits fit in u64 nanoseconds")
}

fn bypasses(directory: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("lifecycle sources are readable") {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            bypasses(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") && path.file_name().is_none_or(|name| name != "executor.rs") {
            let source = fs::read_to_string(&path).expect("source file is readable");
            for crossing in [
                "tokio::spawn",
                "tokio::task::spawn",
                "tokio::time::sleep",
                "tokio::time::timeout",
                "tokio::time::interval",
                "tokio::time::Instant::now",
            ] {
                if source.contains(crossing) {
                    found.push(format!("{}: {crossing}", path.display()));
                }
            }
        }
    }
}

/* === Tests === */

#[test]
fn sources_avoid_tokio() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("nextest sets the manifest directory");
    let source = Path::new(&manifest).join("src");
    assert!(!source.join("work").exists(), "operations own their execution support");
    let mut found = Vec::new();
    for domain in ["operations", "services", "supervision"] {
        bypasses(&source.join(domain), &mut found);
    }
    found.sort();
    assert!(found.is_empty(), "lifecycle bypassed its simulation executor: {found:?}");
}

#[tokio::test(start_paused = true)]
async fn clock_measures_script() {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Measure>("app", Lanes::Open, Arc::new(())).unwrap();
    let scripted = Duration::from_secs(300);

    let answer = runtime
        .operations
        .submit(MeasureContract { wait_nanos: nanos(scripted) })
        .unwrap()
        .await
        .expect("the attempt measures its own wait");

    let scoped = Duration::from_nanos(answer.scoped_nanos);
    assert_eq!(scoped, scripted, "the scope's clock is the one the attempt's wait was taken on");

    // Deterministic, not merely virtual: the same script measures the same
    // duration every time it is run.
    let again = runtime
        .operations
        .submit(MeasureContract { wait_nanos: nanos(scripted) })
        .unwrap()
        .await
        .expect("the second attempt measures its own wait");
    assert_eq!(Duration::from_nanos(again.scoped_nanos), scoped);
    runtime.stop().await.unwrap();
}
