/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

#[path = "../support/native_streams.rs"]
mod native_streams;
use crate::support::fixtures as prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports

// Local Imports
use native_streams::{Fixture, Payload};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("native_consumption_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    verify(Fixture::new().await, 4096).await;
                });
        });
    }
    {
        scenario_runtime::check("executable_consumption_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = prepared_fixtures::artifact("harmos-stream-consumer-fixture");
                    verify(Fixture::executable(&path).await, 1024 * 1024).await;
                });
        });
    }
    #[cfg(target_os = "linux")]
    {
        scenario_runtime::check("shutdown_interrupts_delivery", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = prepared_fixtures::artifact("harmos-stream-consumer-fixture");
                    let fixture = Fixture::executable_paused(&path, Some(1)).await;
                    fixture.runtime.streams.publish(Payload {
                        sequence: 0,
                        bytes: vec![0; 1024 * 1024],
                    });
                    let active = fixture.acknowledged(1).await;
                    assert_eq!(active.bytes, 1024 * 1024);
                    assert_ne!(active.process_id, std::process::id());
                    let process = std::path::PathBuf::from(format!("/proc/{}", active.process_id));
                    assert!(process.exists(), "the original child is active");
                    for sequence in 1..8 {
                        fixture.runtime.streams.publish(Payload {
                            sequence,
                            bytes: vec![sequence as u8; 1024 * 1024],
                        });
                    }
                    tokio::time::timeout(Duration::from_secs(1), fixture.runtime.stop())
                        .await
                        .expect("active delivery and queued rows must not defer shutdown")
                        .unwrap();
                    assert!(!process.exists(), "shutdown joins the original child");
                });
        });
    }
}

async fn verify(fixture: Fixture, size: usize) {
    for batch in 0..2 {
        for offset in 0..8 {
            let sequence = batch * 8 + offset;
            fixture.runtime.streams.publish(Payload {
                sequence,
                bytes: vec![(sequence % 251) as u8; size],
            });
        }
        let expected = (batch + 1) * 8;
        let acknowledged = fixture.acknowledged(expected).await;
        assert_eq!(acknowledged.rows, expected);
        assert_eq!(acknowledged.bytes, expected * size as u64);
    }
    tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .expect("native consumer settles")
        .unwrap();
}

/* === Tests === */
