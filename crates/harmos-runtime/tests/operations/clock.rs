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
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::app::App;
use harmos_runtime::Runtime;
use harmos_runtime::operations::RetryPolicy;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Measured)]
#[derive(Clone)]
struct MeasureContractDeclaration {
    wait_nanos: u64,
}

static MEASURE_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<MeasureContractDeclaration>(
        "clock.measure",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(600),
            attempt_timeout: Duration::from_secs(600),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const MEASURE_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<MeasureContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<MeasureContractDeclaration>::new(&MEASURE_CONTRACT_DECLARATION_DESCRIPTION);

// The Host's Monotonic And Date Readings Across One Wait.
#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Measured {
    scoped_nanos: u64,
    wall_nanos: u64,
}

/* === Enums === */

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(measure_contract_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

async fn measure_contract_declaration_handler(
    request: MeasureContractDeclaration, context: harmos_runtime::operations::FunctionContext<MeasureContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<Measured>, std::convert::Infallible> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<Measured, std::convert::Infallible> = async move {
        let scope = context.scope();
        let scoped = scope.now();
        let wall = scope.clock().date();
        scope.clock().sleep(Duration::from_nanos(request.wait_nanos)).await;
        Ok(Measured {
            scoped_nanos: nanos(scope.now() - scoped),
            wall_nanos: nanos(scope.clock().date().duration_since(wall).unwrap()),
        })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn measure_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        MEASURE_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        measure_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
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
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    crate::support::app::register(&runtime, operations_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    let scripted = Duration::from_secs(300);

    let answer = runtime
        .operation(MeasureContractDeclaration { wait_nanos: nanos(scripted) })
        .start()
        .await
        .unwrap()
        .await
        .expect("the attempt measures its own wait");

    let scoped = Duration::from_nanos(answer.scoped_nanos);
    assert_eq!(scoped, scripted, "the scope's clock is the one the attempt's wait was taken on");

    // Deterministic, not merely virtual: the same script measures the same
    // duration every time it is run.
    let again = runtime
        .operation(MeasureContractDeclaration { wait_nanos: nanos(scripted) })
        .start()
        .await
        .unwrap()
        .await
        .expect("the second attempt measures its own wait");
    assert_eq!(Duration::from_nanos(again.scoped_nanos), scoped);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
