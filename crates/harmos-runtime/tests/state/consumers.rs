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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use support::{Bank, Copied, Ledger, Pulse, booted, catalog, deposit, ledger, pulse, streams, withdraw};
// Local Imports
use crate::state::support;
use harmos_runtime::{
    Runtime, runtime::Incompatible, state::Entry, state::Position, state::Recorded, storage::Batch, storage::Cursor,
    storage::JournalConsumer, storage::Recovery, storage::Source, storage::StreamConsumer,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Asked = Arc<Mutex<Vec<&'static str>>>;

/* === Structs === */

struct Recorder {
    asked: Asked,
    resume: Position,
}

struct Kept {
    cursor: AtomicU64,
    flushed: AtomicU64,
    samples: Mutex<Vec<u64>>,
    gaps: Mutex<Vec<u64>>,
}

struct Tape {
    kept: Arc<Kept>,
}

struct Networked {
    inner: Copied,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Recorder {
    fn new() -> (Self, Asked) {
        let asked: Asked = Arc::new(Mutex::new(Vec::new()));

        (Self { asked: Arc::clone(&asked), resume: Position::ORIGIN }, asked)
    }
}

impl JournalConsumer<Bank> for Recorder {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(&mut self, _entries: Vec<Arc<Entry<Recorded<Bank>>>>, applied: Position) -> Position {
        self.asked.lock().expect("the probe is never poisoned").push("drain");
        self.resume = applied;

        applied
    }

    async fn flush(&mut self) {
        self.asked.lock().expect("the probe is never poisoned").push("flush");
    }
}

impl Tape {
    fn new() -> (Self, Arc<Kept>) {
        let kept = Arc::new(Kept {
            cursor: AtomicU64::new(0),
            flushed: AtomicU64::new(0),
            samples: Mutex::new(Vec::new()),
            gaps: Mutex::new(Vec::new()),
        });

        (Self::resuming(&kept), kept)
    }

    fn resuming(kept: &Arc<Kept>) -> Self {
        Self { kept: Arc::clone(kept) }
    }
}

impl StreamConsumer<Bank> for Tape {
    fn resume(&self) -> Cursor {
        Cursor(self.kept.cursor.load(Ordering::Acquire))
    }

    fn selections(&self) -> Vec<harmos_runtime::streams::Definition> {
        vec![Pulse::definition()]
    }

    async fn drain(&mut self, batch: Batch<Bank>) -> Cursor {
        let resume = self.resume();
        let reached = batch.reaches();
        let samples: Vec<u64> = batch.select::<Pulse>().map(|row| row.value.sample).collect();
        let missed = (reached.0 - resume.0) - samples.len() as u64;

        if missed > 0 {
            self.kept.gaps.lock().expect("the tape is never poisoned").push(missed);
        }
        self.kept.samples.lock().expect("the tape is never poisoned").extend(samples);
        self.kept.cursor.store(reached.0, Ordering::Release);

        reached
    }

    async fn flush(&mut self) {
        self.kept.flushed.fetch_add(1, Ordering::AcqRel);
    }
}

impl JournalConsumer<Bank> for Networked {
    fn resume(&self) -> Position {
        JournalConsumer::resume(&self.inner)
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<Bank>>>>, applied: Position) -> Position {
        self.inner.drain(entries, applied).await
    }
}

impl Source<Bank> for Networked {
    async fn load(&mut self) -> Result<Recovery<Bank>, Incompatible> {
        // A network source suspends before it answers.
        tokio::task::yield_now().await;
        self.inner.load().await
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn drain_precedes_flush() {
    let (recorder, asked) = Recorder::new();
    let runtime = booted(Ledger::default(), |builder| builder.attach(recorder)).await;

    for amount in [10, 20] {
        runtime.state.commit(deposit(amount)).await.expect("accepted");
    }
    let applied = runtime.state.applied();
    let settled = runtime.stop().await.unwrap();

    // `stop` answered, so the join came last; what the probe wrote down is
    // everything that happened before it.
    let asked = asked.lock().expect("the probe is never poisoned").clone();

    assert_eq!(settled, applied, "the frontier stop reports is read back after the flush");
    assert!(
        asked.ends_with(&["drain", "flush", "drain"]),
        "the final drain precedes the flush, and the one drain after it is \
         the read-back of what the flush made durable: {asked:?}"
    );
    assert_eq!(asked.iter().filter(|verb| **verb == "flush").count(), 1, "flush happens once, at stop, never per drain");
}

#[tokio::test]
async fn recorder_resumes_own() {
    let (tape, kept) = Tape::new();

    let runtime = recording(tape).await;
    for sample in 1..=5 {
        runtime.streams.publish(pulse(sample));
    }
    runtime.stop().await.unwrap();

    assert_eq!(kept.cursor.load(Ordering::Acquire), 5, "the first supervisor made every published row durable");
    assert_eq!(*kept.samples.lock().expect("the tape is never poisoned"), vec![1, 2, 3, 4, 5]);
    assert_eq!(kept.flushed.load(Ordering::Acquire), 1, "a stream recorder rides the same stop order as every other fold");

    let runtime = recording(Tape::resuming(&kept)).await;
    for sample in 6..=9 {
        runtime.streams.publish(pulse(sample));
    }
    runtime.stop().await.unwrap();

    assert_eq!(kept.cursor.load(Ordering::Acquire), 9, "new rows continue after the durable cursor instead of reusing it");
    assert_eq!(
        *kept.samples.lock().expect("the tape is never poisoned"),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        "reattachment neither skips nor repeats a row"
    );
    assert_eq!(kept.flushed.load(Ordering::Acquire), 2, "every attachment is flushed by its own stop");
}

#[tokio::test]
async fn gap_reports_window() {
    let (tape, kept) = Tape::new();
    let runtime = recording(tape).await;

    // No await lets the supervisor run until all twelve rows have reached the
    // eight-row ring, so the first delivery must account for four evictions.
    for sample in 1..=12 {
        runtime.streams.publish(pulse(sample));
    }
    runtime.stop().await.unwrap();

    assert_eq!(
        *kept.gaps.lock().expect("the tape is never poisoned"),
        vec![4],
        "(reaches - resume) - delivered rows is the exact missed count"
    );
    assert_eq!(
        *kept.samples.lock().expect("the tape is never poisoned"),
        (5..=12).collect::<Vec<_>>(),
        "delivery continues from the oldest retained row"
    );
}

#[tokio::test]
async fn recovery_accepts_source() {
    let (copy, stored) = Copied::new();
    let live = booted(Ledger::default(), |builder| builder.recover(copy)).await;

    live.state.commit(deposit(40)).await.expect("accepted");
    live.state.commit(withdraw(15)).await.expect("accepted");
    let expected = ledger(&live).await;
    let frontier = live.state.applied();
    live.stop().await.unwrap();

    let recovered = booted(Ledger::default(), |builder| builder.recover(Networked { inner: Copied::resuming(&stored) })).await;

    assert_eq!(ledger(&recovered).await, expected);
    assert_eq!(recovered.state.applied(), frontier);
}

async fn recording(tape: Tape) -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .attach_streams(tape)
        .runtime()
        .await
        .expect("the catalog and attachment are honorable")
}

/* === Tests === */
