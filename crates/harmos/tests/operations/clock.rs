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
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos::Runtime;
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Measure;

/* === Enums === */
#[harmos::operation(
        request = Duration,
        response = Duration,
        id = "clock.measure",
        policy = OperationPolicy {
            timeout: Duration::from_secs(600),
            attempt_timeout: Duration::from_secs(600),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct MeasureContract;

#[harmos::catalog]
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

    async fn initialize<'ctx>(
        _resources: Arc<()>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        wait: &mut Duration,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Duration>, Self::Error> {
        let result: ::core::result::Result<Duration, Self::Error> = async move {
            let scope = context.scope();
            let scoped = scope.now();
            tokio::time::sleep(*wait).await;
            Ok(scope.now() - scoped)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn bypasses(directory: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("lifecycle sources are readable") {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            bypasses(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path.file_name().is_none_or(|name| name != "executor.rs")
        {
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
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(
        !source.join("work").exists(),
        "operations own their execution support"
    );
    let mut found = Vec::new();
    for domain in ["operations", "services", "supervision"] {
        bypasses(&source.join(domain), &mut found);
    }
    found.sort();
    assert!(
        found.is_empty(),
        "lifecycle bypassed its simulation executor: {found:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn clock_measures_script() {
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Measure>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    let scripted = Duration::from_secs(300);

    let answer = runtime
        .operations
        .submit_as::<MeasureContract>(scripted)
        .unwrap()
        .await
        .expect("the attempt measures its own wait");

    let scoped = *answer;
    assert_eq!(
        scoped, scripted,
        "the scope's clock is the one the attempt's wait was taken on"
    );

    // Deterministic, not merely virtual: the same script measures the same
    // duration every time it is run.
    let again = runtime
        .operations
        .submit_as::<MeasureContract>(scripted)
        .unwrap()
        .await
        .expect("the second attempt measures its own wait");
    assert_eq!(*again, scoped);
    runtime.stop().await.unwrap();
}
