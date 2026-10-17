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
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use support::{Bank, Change, Ledger, booted, catalog, deposit};
// Local Imports
use crate::support;
use harmos_runtime::{
    Error, Runtime, runtime::Incompatible, state::Entry, state::Position, state::Recorded,
    storage::Horizon, storage::JournalConsumer, storage::Recovery, storage::Source,
};
use schematic_storage::Snapshot;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Prunable {
    taken: Position,
    take: bool,
    advance: Arc<AtomicBool>,
    durable: Arc<AtomicU64>,
    pruned: Arc<Mutex<Vec<Position>>>,
    batches: Arc<Mutex<Vec<Vec<Position>>>>,
}

struct Watched {
    advance: Arc<AtomicBool>,
    pruned: Arc<Mutex<Vec<Position>>>,
    batches: Arc<Mutex<Vec<Vec<Position>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Prunable {
    fn controlled(advance: bool) -> (Self, Watched) {
        Self::new(true, advance)
    }

    fn stalled() -> (Self, Watched) {
        Self::new(false, false)
    }

    fn new(take: bool, advance: bool) -> (Self, Watched) {
        let watched = Watched {
            advance: Arc::new(AtomicBool::new(advance)),
            pruned: Arc::new(Mutex::new(Vec::new())),
            batches: Arc::new(Mutex::new(Vec::new())),
        };
        let durable = Arc::new(AtomicU64::new(0));
        let source = Self {
            taken: Position::ORIGIN,
            take,
            advance: Arc::clone(&watched.advance),
            durable,
            pruned: Arc::clone(&watched.pruned),
            batches: Arc::clone(&watched.batches),
        };

        (source, watched)
    }
}

impl JournalConsumer<Bank> for Prunable {
    fn resume(&self) -> Position {
        self.taken
    }

    async fn drain(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        _applied: Position,
    ) -> Position {
        self.batches
            .lock()
            .expect("the probe is never poisoned")
            .push(entries.iter().map(|entry| entry.position).collect());
        if self.take
            && let Some(last) = entries.last()
        {
            self.taken = last.position;
        }
        if self.advance.load(Ordering::Acquire) {
            self.durable.store(self.taken.0, Ordering::Release);
        }

        Position(self.durable.load(Ordering::Acquire))
    }
}

impl Source<Bank> for Prunable {
    async fn load(&mut self) -> Result<Recovery<Bank>, Incompatible> {
        Ok(Recovery::default())
    }

    async fn prune(&mut self, beneath: Position) {
        self.pruned
            .lock()
            .expect("the probe is never poisoned")
            .push(beneath);
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("snapshot_blocks_pruning", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (source, watched) = Prunable::controlled(true);
                    let failing = Arc::new(AtomicBool::new(true));
                    let attempts = Arc::new(AtomicUsize::new(0));
                    let snapshot =
                        Snapshot::<Bank>::folding(Ledger::default(), Position::ORIGIN, {
                            let failing = Arc::clone(&failing);
                            let attempts = Arc::clone(&attempts);
                            move |_, _| {
                                attempts.fetch_add(1, Ordering::AcqRel);
                                if failing.load(Ordering::Acquire) {
                                    Err(io::Error::other("injected snapshot failure"))
                                } else {
                                    Ok(())
                                }
                            }
                        });
                    let runtime = booted(Ledger::default(), |builder| {
                        builder
                            .recover(source)
                            .snapshot(snapshot)
                            .retain(Horizon::BeneathSnapshot)
                            .tail(1)
                    })
                    .await;

                    let first = runtime.state.commit(deposit(1)).await.expect("accepted");
                    runtime.state.wait_until_stored(first.position).await;
                    wait_for(|| attempts.load(Ordering::Acquire) > 0).await;
                    assert!(watched.pruned.lock().expect("not poisoned").is_empty());

                    failing.store(false, Ordering::Release);
                    let second = runtime.state.commit(deposit(1)).await.expect("accepted");
                    runtime.state.wait_until_stored(second.position).await;
                    wait_for(|| last_prune(&watched) == Some(second.position)).await;

                    assert!(matches!(
                        runtime.state.query::<Change>(Position::ORIGIN).await.next(),
                        Some(Err(Error::BeyondTail))
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("checkpoint_blocks_pruning", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (source, watched) = Prunable::controlled(false);
                    let writes = Arc::new(AtomicUsize::new(0));
                    let snapshot =
                        Snapshot::<Bank>::folding(Ledger::default(), Position::ORIGIN, {
                            let writes = Arc::clone(&writes);
                            move |_, _| {
                                writes.fetch_add(1, Ordering::AcqRel);
                                Ok(())
                            }
                        });
                    let runtime = booted(Ledger::default(), |builder| {
                        builder
                            .recover(source)
                            .snapshot(snapshot)
                            .retain(Horizon::BeneathSnapshot)
                    })
                    .await;

                    runtime.state.commit(deposit(1)).await.expect("accepted");
                    wait_for(|| writes.load(Ordering::Acquire) > 0).await;
                    assert!(watched.pruned.lock().expect("not poisoned").is_empty());

                    watched.advance.store(true, Ordering::Release);
                    let recovered = runtime.state.commit(deposit(1)).await.expect("accepted");
                    runtime.state.wait_until_stored(recovered.position).await;
                    wait_for(|| last_prune(&watched) == Some(recovered.position)).await;
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("snapshot_requires_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let started = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .retain(Horizon::BeneathSnapshot)
                        .runtime()
                        .await;
                    let Err(refused) = started else {
                        panic!("retention without its snapshot must be refused at assembly");
                    };

                    assert!(
                        refused
                            .to_string()
                            .contains("requires a snapshot declaration")
                    );
                });
        });
    }
    {
        scenario_runtime::check("storage_ac006_declared", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let beneath = run_stalled(2).await;
                    let within = run_stalled(3).await;

                    assert_eq!(beneath.iter().rev().nth(1), Some(&Vec::new()));
                    assert_eq!(
                        within.iter().rev().nth(1),
                        Some(&vec![Position(1), Position(2), Position(3)])
                    );
                });
        });
    }
}

async fn run_stalled(tail: usize) -> Vec<Vec<Position>> {
    let (source, watched) = Prunable::stalled();
    let runtime = booted(Ledger::default(), |builder| {
        builder.recover(source).tail(tail)
    })
    .await;
    for _ in 0..3 {
        runtime.state.commit(deposit(1)).await.expect("accepted");
    }
    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);

    watched.batches.lock().expect("not poisoned").clone()
}

async fn wait_for(done: impl Fn() -> bool) {
    for _ in 0..10_000 {
        if done() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the supervisor did not reach the expected frontier");
}

fn last_prune(watched: &Watched) -> Option<Position> {
    watched.pruned.lock().expect("not poisoned").last().copied()
}

/* === Tests === */
