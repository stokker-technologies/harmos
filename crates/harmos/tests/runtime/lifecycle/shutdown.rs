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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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

use super::*;
use crate::probe::{Editor, Sheet};
use harmos::{Entry, JournalConsumer, Recorded};
use std::sync::atomic::AtomicUsize;
use tokio::sync::Notify;

struct HeldDrain {
    attempts: Arc<AtomicUsize>,
    release: Arc<Notify>,
}
impl JournalConsumer<Editor> for HeldDrain {
    fn resume(&self) -> Position {
        Position::ORIGIN
    }
    async fn drain(&mut self, _: Vec<Arc<Entry<Recorded<Editor>>>>, applied: Position) -> Position {
        applied
    }
    async fn flush(&mut self) {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.release.notified().await;
    }
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("budget_retains_journal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let attempts = Arc::new(AtomicUsize::new(0));
                    let release = Arc::new(Notify::new());
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .attach(HeldDrain {
                            attempts: attempts.clone(),
                            release: release.clone(),
                        })
                        .runtime()
                        .await
                        .unwrap();
                    assert!(matches!(
                        runtime.stop_with_timeout(Duration::ZERO).await,
                        Err(Error::InvalidInput {
                            field: "runtime shutdown timeout",
                            ..
                        })
                    ));
                    let accepted = runtime
                        .state
                        .commit(crate::probe::rename("open"))
                        .await
                        .unwrap();
                    assert_eq!(accepted.position, Position(1));
                    for _ in 0..2 {
                        assert!(matches!(
                            runtime.stop_with_timeout(Duration::from_millis(10)).await,
                            Err(Error::ShutdownTimeout { phase: "journal" })
                        ));
                        assert_eq!(attempts.load(Ordering::SeqCst), 1);
                    }
                    release.notify_one();
                    assert_eq!(
                        runtime
                            .stop_with_timeout(Duration::from_secs(1))
                            .await
                            .unwrap(),
                        Position(1)
                    );
                    assert_eq!(runtime.stop().await.unwrap(), Position(1));
                    assert_eq!(attempts.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("shutdown_budget_shared", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let attempts = Arc::new(AtomicUsize::new(0));
                    let release = Arc::new(Notify::new());
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .attach(HeldDrain {
                            attempts: attempts.clone(),
                            release: release.clone(),
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let participant = runtime.stop_participant();
                    let leaving = tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(6)).await;
                        drop(participant);
                    });
                    let started = tokio::time::Instant::now();
                    assert!(matches!(
                        runtime.stop_with_timeout(Duration::from_millis(10)).await,
                        Err(Error::ShutdownTimeout { phase: "journal" })
                    ));
                    assert_eq!(started.elapsed(), Duration::from_millis(10));
                    leaving.await.unwrap();
                    assert_eq!(attempts.load(Ordering::SeqCst), 1);
                    release.notify_one();
                    runtime.stop().await.unwrap();
                });
        });
    }
}
