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
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
use support::{Bank, Change, Ledger, booted, catalog, deposit};
// Local Imports
use crate::streams::support;
use harmos_runtime::{
    Error, Runtime, runtime::Incompatible, state::Entry, state::Position, state::Recorded,
    storage::Horizon, storage::JournalConsumer, storage::Recovery, storage::Source,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Save = Box<dyn FnMut() -> io::Result<()> + Send>;

/* === Structs === */
// A Snapshot Is Durable Exactly As Far As Its Last Successful Save.
struct Snapshot {
    taken: Position,
    durable: Position,
    save: Save,
}

struct Prunable {
    taken: Position,
    take: bool,
    advance: Arc<AtomicBool>,
    durable: Arc<AtomicU64>,
    pruned: watch::Sender<Option<Position>>,
    batches: Arc<Mutex<Vec<Vec<Position>>>>,
}

struct Watched {
    advance: Arc<AtomicBool>,
    pruned: watch::Sender<Option<Position>>,
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
            pruned: watch::Sender::new(None),
            batches: Arc::new(Mutex::new(Vec::new())),
        };
        let durable = Arc::new(AtomicU64::new(0));
        let source = Self {
            taken: Position::ORIGIN,
            take,
            advance: Arc::clone(&watched.advance),
            durable,
            pruned: watched.pruned.clone(),
            batches: Arc::clone(&watched.batches),
        };

        (source, watched)
    }
}

impl Snapshot {
    fn new(save: impl FnMut() -> io::Result<()> + Send + 'static) -> Self {
        Self {
            taken: Position::ORIGIN,
            durable: Position::ORIGIN,
            save: Box::new(save),
        }
    }
}

impl JournalConsumer<Bank> for Snapshot {
    fn resume(&self) -> Position {
        self.taken
    }

    async fn drain(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        _applied: Position,
    ) -> Position {
        if let Some(last) = entries.last() {
            self.taken = last.position;
        }
        if self.durable < self.taken && (self.save)().is_ok() {
            self.durable = self.taken;
        }

        self.durable
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
        self.pruned.send_replace(Some(beneath));
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn snapshot_blocks_pruning() {
    let (source, watched) = Prunable::controlled(true);
    let failing = Arc::new(AtomicBool::new(true));
    let attempts = watch::Sender::new(0_usize);
    let snapshot = Snapshot::new({
        let failing = Arc::clone(&failing);
        let attempts = attempts.clone();
        move || {
            attempts.send_modify(|attempts| *attempts += 1);
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
    saved(&attempts).await;
    assert_eq!(*watched.pruned.borrow(), None);

    failing.store(false, Ordering::Release);
    let second = runtime.state.commit(deposit(1)).await.expect("accepted");
    runtime.state.wait_until_stored(second.position).await;
    pruned(&watched, second.position).await;

    assert!(matches!(
        runtime.state.query::<Change>(Position::ORIGIN).await.next(),
        Some(Err(Error::BeyondTail))
    ));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn checkpoint_blocks_pruning() {
    let (source, watched) = Prunable::controlled(false);
    let writes = watch::Sender::new(0_usize);
    let snapshot = Snapshot::new({
        let writes = writes.clone();
        move || {
            writes.send_modify(|writes| *writes += 1);
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
    saved(&writes).await;
    assert_eq!(*watched.pruned.borrow(), None);

    watched.advance.store(true, Ordering::Release);
    let recovered = runtime.state.commit(deposit(1)).await.expect("accepted");
    runtime.state.wait_until_stored(recovered.position).await;
    pruned(&watched, recovered.position).await;
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn retention_requires_snapshot() {
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
}

#[tokio::test]
async fn stalled_redelivery_tail() {
    let beneath = run_stalled(2).await;
    let within = run_stalled(3).await;

    assert_eq!(beneath.iter().rev().nth(1), Some(&Vec::new()));
    assert_eq!(
        within.iter().rev().nth(1),
        Some(&vec![Position(1), Position(2), Position(3)])
    );
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

async fn saved(attempts: &watch::Sender<usize>) {
    attempts
        .subscribe()
        .wait_for(|attempts| *attempts > 0)
        .await
        .expect("the snapshot keeps its counter");
}

async fn pruned(watched: &Watched, beneath: Position) {
    watched
        .pruned
        .subscribe()
        .wait_for(|pruned| *pruned == Some(beneath))
        .await
        .expect("the source keeps its prune record");
}

/* === Tests === */
