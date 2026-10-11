/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use minibench_app::operations::{Corrected, ReportRequest};
use minibench_app::resources::Resources;
use minibench_app::streams::Reading;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("report_uses_initialized", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let owner = std::sync::Arc::new(Resources {
                        device: "bench".into(),
                        calibration: minibench_app::resources::Calibration::measured([(2, 10)]),
                    });
                    let runtime = minibench_app::app::builder((), std::sync::Arc::clone(&owner))
                        .runtime()
                        .await
                        .unwrap();
                    let report = runtime
                        .operations
                        .call::<minibench_app::operations::Report>(ReportRequest {
                            readings: vec![Reading {
                                device: "captured".into(),
                                channel: 2,
                                millivolts: 3320,
                            }],
                            channel: 2,
                        })
                        .await
                        .unwrap();
                    assert_eq!(
                        *report,
                        Corrected {
                            device: "bench".into(),
                            channel: 2,
                            millivolts: 3310
                        }
                    );
                    let error = runtime
                        .operations
                        .call::<minibench_app::operations::Report>(ReportRequest {
                            readings: vec![],
                            channel: 7,
                        })
                        .await
                        .err()
                        .unwrap();
                    assert!(matches!(error, harmos::Error::OperationFailed { .. }));
                    runtime.stop().await.unwrap();
                });
        });
    }
}
