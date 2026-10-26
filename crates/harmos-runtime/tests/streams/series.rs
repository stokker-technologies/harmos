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
use std::any::Any;
use std::ops::Bound;
use std::sync::{Arc, Mutex};
// Third Party Imports
use support::{Bank, Capture, Ledger, capture, catalog, deposit, streams};
// Local Imports
use crate::streams::support;
use harmos_runtime::{
    Aggregate, Runtime, state::At, state::Position, storage::Batch, storage::Cursor, storage::Series, storage::StreamConsumer,
    streams::Aggregated, streams::Axial, streams::BeneathWindow, streams::Bucket, streams::Definition, streams::Point, streams::Prefix,
    streams::Stream, streams::Tracked,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Kept = Arc<Mutex<Vec<At<Arc<Capture>>>>>;

type Asked = Arc<Mutex<Vec<(String, Vec<String>)>>>;

type Budgets = Arc<Mutex<Vec<usize>>>;

type Watched = (Kept, Asked, Budgets);

/* === Structs === */

struct Tape {
    rows: Kept,
    asked: Asked,
    cursor: Cursor,
    floor: Position,
    budgets: Budgets,
    sealed: Position,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Tape {
    fn new() -> (Self, Watched) {
        Self::retaining_above(Position::ORIGIN)
    }

    fn retaining_above(floor: Position) -> (Self, Watched) {
        let rows = Arc::new(Mutex::new(Vec::new()));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let budgets = Arc::new(Mutex::new(Vec::new()));

        (
            Self {
                rows: Arc::clone(&rows),
                asked: Arc::clone(&asked),
                cursor: Cursor::ORIGIN,
                floor,
                budgets: Arc::clone(&budgets),
                sealed: Position::ORIGIN,
            },
            (rows, asked, budgets),
        )
    }
}

impl StreamConsumer<Bank> for Tape {
    fn resume(&self) -> Cursor {
        self.cursor
    }

    fn selections(&self) -> Vec<Definition> {
        vec![<Capture as Stream<Bank>>::definition()]
    }

    async fn drain(&mut self, batch: Batch<Bank>) -> Cursor {
        let shared: Vec<_> = batch.select_shared::<Capture>().collect();
        for (borrowed, retained) in batch.select::<Capture>().zip(&shared) {
            assert!(std::ptr::eq(borrowed.value, Arc::as_ptr(&retained.value)));
        }
        self.rows.lock().expect("the tape is never poisoned").extend(shared);
        self.cursor = batch.reaches();

        self.cursor
    }
}

impl Series<Bank> for Tape {
    async fn rows(
        &mut self, stream: Definition, track: &[String], start: Bound<Position>, end: Bound<Position>,
    ) -> Result<Vec<At<Arc<dyn Any + Send + Sync>>>, BeneathWindow> {
        self.asked.lock().expect("the tape is never poisoned").push((stream.id().to_owned(), track.to_vec()));

        if reaches_beneath(start, self.floor) {
            return Err(BeneathWindow::Floor { floor: self.floor });
        }

        Ok(self
            .rows
            .lock()
            .expect("the tape is never poisoned")
            .iter()
            .filter(|At { position, .. }| admitted(start, end, *position))
            .filter(|At { value, .. }| narrowed(track, value))
            .map(|At { value, position }| At { value: Arc::clone(value) as Arc<dyn Any + Send + Sync>, position: *position })
            .collect())
    }

    async fn buckets(
        &mut self, stream: Definition, track: &[String], start: Bound<u64>, end: Bound<u64>, budget: usize,
    ) -> Result<Vec<Bucket>, BeneathWindow> {
        self.asked.lock().expect("the tape is never poisoned").push((stream.id().to_owned(), track.to_vec()));
        self.budgets.lock().expect("the tape is never poisoned").push(budget);

        // The coarsest grouping that fits the budget, which is what a level
        // choice is: four points per bucket, so the budget is a bucket count
        // times four.
        let rows: Vec<At<Arc<Capture>>> = self
            .rows
            .lock()
            .expect("the tape is never poisoned")
            .iter()
            .filter(|At { position, .. }| *position <= self.sealed)
            .filter(|At { value, .. }| within(start, end, value.axis()))
            .filter(|At { value, .. }| narrowed(track, value))
            .cloned()
            .collect();
        let buckets = (budget / 4).max(1);
        let per = rows.len().div_ceil(buckets).max(1);

        Ok(rows
            .chunks(per)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|At { value, .. }| one(value))
                    .reduce(|held, next| held.merge(&next))
                    .expect("a chunk folds at least one row")
            })
            .collect())
    }

    async fn aggregate(
        &mut self, stream: Definition, track: &[String], start: Bound<u64>, end: Bound<u64>, kind: Aggregate,
    ) -> Result<Vec<Aggregated>, BeneathWindow> {
        self.asked.lock().expect("the tape is never poisoned").push((stream.id().to_owned(), track.to_vec()));

        // One measured field folded into one bucket. A bucket answers the
        // mergeable members; median and percentiles need the values, which
        // this tape does not serve, so they answer nothing.
        let answer = self
            .rows
            .lock()
            .expect("the tape is never poisoned")
            .iter()
            .filter(|At { position, .. }| *position <= self.sealed)
            .filter(|At { value, .. }| within(start, end, value.axis()))
            .filter(|At { value, .. }| narrowed(track, value))
            .map(|At { value, .. }| one(value))
            .reduce(|held, next| held.merge(&next))
            .and_then(|folded| folded.aggregate(kind));

        Ok(answer.map(|value| Aggregated { field: "value".to_owned(), value }).into_iter().collect())
    }

    async fn seal(&mut self, _stream: Definition, _track: &[String]) -> Position {
        self.sealed = self
            .rows
            .lock()
            .expect("the tape is never poisoned")
            .iter()
            .map(|At { position, .. }| *position)
            .max()
            .unwrap_or(Position::ORIGIN);

        self.sealed
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn narrowed(track: &[String], row: &Capture) -> bool {
    Prefix::<<Capture as Tracked>::Track>::labels(&row.track()).starts_with(track)
}

fn one(row: &Capture) -> Bucket {
    at(row.axis(), row.value as f64)
}

fn at(axis: u64, value: f64) -> Bucket {
    Bucket::point("value", Point { axis, value })
}

fn within(start: Bound<u64>, end: Bound<u64>, at: u64) -> bool {
    let above = match start {
        Bound::Included(from) => at >= from,
        Bound::Excluded(from) => at > from,
        Bound::Unbounded => true,
    };
    let below = match end {
        Bound::Included(to) => at <= to,
        Bound::Excluded(to) => at < to,
        Bound::Unbounded => true,
    };

    above && below
}

fn reaches_beneath(start: Bound<Position>, floor: Position) -> bool {
    match start {
        Bound::Included(at) => at <= floor,
        Bound::Excluded(at) => at < floor,
        Bound::Unbounded => floor > Position::ORIGIN,
    }
}

fn admitted(start: Bound<Position>, end: Bound<Position>, at: Position) -> bool {
    let above = match start {
        Bound::Included(from) => at >= from,
        Bound::Excluded(from) => at > from,
        Bound::Unbounded => true,
    };
    let below = match end {
        Bound::Included(to) => at <= to,
        Bound::Excluded(to) => at < to,
        Bound::Unbounded => true,
    };

    above && below
}

async fn resumed(tape: Tape, at: Position) -> Runtime<Bank> {
    Runtime::<Bank>::builder(At { value: Ledger::default(), position: at }, ())
        .register(catalog())
        .register(streams())
        .series(tape)
        .runtime()
        .await
        .expect("a boot standing on a recovered frontier")
}

async fn served(tape: Tape) -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .series(tape)
        .runtime()
        .await
        .expect("a memory-only boot recovers nothing")
}

async fn captured(rows: &Kept, count: usize) {
    for _ in 0..10_000 {
        if rows.lock().expect("the tape is never poisoned").len() >= count {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("delivery never reached {count} rows");
}

async fn publish(runtime: &Runtime<Bank>, rows: &Kept, ordinals: &[u64]) {
    for (taken, ordinal) in ordinals.iter().enumerate() {
        runtime.state.commit(deposit(1)).await.expect("the deposit is accepted");
        runtime.streams.publish(capture(1, 1, *ordinal, *ordinal as i64));
        captured(rows, taken + 1).await;
    }
}

#[tokio::test]
async fn range_beneath_window() {
    let (tape, (rows, asked, _budgets)) = Tape::new();
    let runtime = served(tape).await;

    // Twelve rows through a window of four: eight are answerable only from
    // the tier beneath it.
    publish(&runtime, &rows, &(1..=12).collect::<Vec<_>>()).await;

    let answered: Vec<u64> = runtime
        .streams
        .query::<Capture>(..)
        .await
        .expect("the series tier holds what the window evicted")
        .map(|At { value, .. }| value.ordinal)
        .collect();

    assert_eq!(answered, (1..=12).collect::<Vec<_>>(), "every row is answered exactly once, in publish order, across the seam");
    assert_eq!(
        asked.lock().expect("the tape is never poisoned").as_slice(),
        [("capture".to_owned(), Vec::new())],
        "the served tier is asked once, and asked about the stream by name"
    );

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn retained_skips_tier() {
    let (tape, (rows, asked, _budgets)) = Tape::new();
    let (second, (second_rows, _, _)) = Tape::new();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .series(tape)
        .attach_streams(second)
        .runtime()
        .await
        .unwrap();

    publish(&runtime, &rows, &[1, 2]).await;
    captured(&second_rows, 2).await;
    let original: Vec<_> = rows.lock().unwrap().iter().map(|row| Arc::clone(&row.value)).collect();
    let shared: Vec<_> = second_rows.lock().unwrap().iter().map(|row| Arc::clone(&row.value)).collect();
    assert_eq!(original.len(), shared.len());
    for (first, second) in original.iter().zip(&shared) {
        assert!(Arc::ptr_eq(first, second));
    }

    let answered: Vec<u64> =
        runtime.streams.query::<Capture>(..).await.expect("nothing was evicted").map(|At { value, .. }| value.ordinal).collect();

    assert_eq!(answered, vec![1, 2]);
    assert!(
        asked.lock().expect("the tape is never poisoned").is_empty(),
        "the window answered the whole range, so nothing was asked beneath it"
    );

    runtime.stop().await.unwrap();
    drop((runtime, rows, original, second_rows));
    assert_eq!(shared.iter().map(|row| row.ordinal).collect::<Vec<_>>(), [1, 2]);
}

#[tokio::test]
async fn window_stands_durable() {
    let (tape, (rows, asked, _budgets)) = Tape::new();

    // What a previous run left in the tier. This boot's ring never saw these
    // rows and never will, so a window that answered for the whole of history
    // until it had evicted something would call them missing.
    rows.lock().expect("the tape is never poisoned").extend([
        At { value: Arc::new(capture(1, 1, 1, 1)), position: Position(1) },
        At { value: Arc::new(capture(1, 1, 2, 2)), position: Position(2) },
    ]);
    let runtime = resumed(tape, Position(2)).await;

    let answered: Vec<u64> = runtime
        .streams
        .query::<Capture>(..)
        .await
        .expect("the tier beneath holds the pre-boot history")
        .map(|At { value, .. }| value.ordinal)
        .collect();

    assert_eq!(answered, vec![1, 2], "the first query of a restart splices, before the ring has evicted anything");
    assert_eq!(
        asked.lock().expect("the tape is never poisoned").as_slice(),
        [("capture".to_owned(), Vec::new())],
        "an empty ring standing on a watermark reaches the tier exactly once"
    );

    // The live half is still the window's. A row published above that floor is
    // answered from the ring, once, beside the durable prefix beneath it.
    publish(&runtime, &rows, &[3]).await;

    let answered: Vec<u64> =
        runtime.streams.query::<Capture>(..).await.expect("both tiers answer").map(|At { value, .. }| value.ordinal).collect();

    assert_eq!(answered, vec![1, 2, 3], "the boundary is the watermark: beneath it the tier's, above it the ring's");

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn floor_lacks_tier() {
    let (tape, (rows, _asked, _budgets)) = Tape::retaining_above(Position(6));
    let runtime = served(tape).await;

    publish(&runtime, &rows, &(1..=12).collect::<Vec<_>>()).await;

    assert_eq!(
        runtime.streams.query::<Capture>(..).await.expect_err("the adapter's own history begins later than the range"),
        BeneathWindow::Floor { floor: Position(6) },
        "the floor is evidence, not a shortened answer"
    );

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn tracked_read_narrows() {
    let (tape, (rows, asked, _budgets)) = Tape::new();
    let runtime = served(tape).await;

    // Two runs of two probes each, interleaved, so a partition is never merely
    // a prefix of the published order.
    for ordinal in 1..=8u64 {
        runtime.state.commit(deposit(1)).await.expect("the deposit is accepted");
        runtime.streams.publish(capture(ordinal % 2, ordinal % 4 / 2, ordinal, ordinal as i64));
        captured(&rows, ordinal as usize).await;
    }

    let answered: Vec<u64> = runtime
        .streams
        .query::<Capture>(..)
        .track((support::Run(1), support::Probe(1)))
        .await
        .expect("the series tier holds what the window evicted")
        .map(|At { value, .. }| value.ordinal)
        .collect();
    let subtree: Vec<u64> = runtime
        .streams
        .query::<Capture>(..)
        .track((support::Run(1),))
        .await
        .expect("a prefix of the track selects the subtree beneath it")
        .map(|At { value, .. }| value.ordinal)
        .collect();

    assert_eq!(answered, vec![3, 7], "the answer is cut by the application's own track equality, component by component");
    assert_eq!(subtree, vec![1, 3, 5, 7], "a prefix answers for every partition beneath it");
    assert_eq!(
        asked.lock().expect("the tape is never poisoned").as_slice(),
        [("capture".to_owned(), vec!["run_1".to_owned(), "probe_1".to_owned()]), ("capture".to_owned(), vec!["run_1".to_owned()]),],
        "the component labels of the selection are what narrow the served read"
    );

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn survey_spends_caller() {
    let (tape, (rows, asked, budgets)) = Tape::new();
    let runtime = served(tape).await;

    publish(&runtime, &rows, &(1..=12).collect::<Vec<_>>()).await;
    let sealed = runtime.streams.seal::<Capture>().await;

    let buckets = runtime.streams.survey::<Capture>(..).budget(12).await.expect("the served tier holds the sealed rows");

    assert_eq!(*budgets.lock().expect("the tape is never poisoned"), vec![12], "the caller's ceiling reaches the adapter unchanged");
    assert_eq!(asked.lock().expect("the tape is never poisoned").as_slice(), [("capture".to_owned(), Vec::new())]);
    assert_eq!(buckets.len(), 3, "twelve points of budget is three buckets");
    assert_eq!(buckets.iter().map(Bucket::count).sum::<u64>(), 12, "every sealed row is folded into exactly one bucket");
    let whole = buckets.iter().skip(1).fold(buckets[0].clone(), |held, next| held.merge(next));
    assert_eq!(whole.aggregate(Aggregate::Count), Some(12.0));
    assert_eq!(whole.aggregate(Aggregate::Min), Some(whole.min.value), "the summary and the drawn points agree");
    assert_eq!(whole.aggregate(Aggregate::Median), None, "a bucket keeps no values to order");
    assert_eq!(buckets[0].first, Point { axis: 1, value: 1.0 }, "the extremes carry the axis coordinates they occurred at");
    assert_eq!(
        buckets.iter().cloned().reduce(|held, next| held.merge(&next)).expect("a survey answers at least one bucket").max,
        Point { axis: 12, value: 12.0 },
        "merging the answer back down reproduces the whole range's extreme"
    );
    assert!(sealed > Position::ORIGIN);

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn seal_reports_position() {
    let (tape, (rows, _asked, _budgets)) = Tape::new();
    let runtime = served(tape).await;

    assert_eq!(runtime.streams.seal::<Capture>().await, Position::ORIGIN, "a tape that has taken nothing has made nothing durable");

    publish(&runtime, &rows, &(1..=6).collect::<Vec<_>>()).await;
    let sealed = runtime.streams.seal::<Capture>().await;

    assert_eq!(sealed, runtime.state.applied(), "the barrier reaches the last row published, which was stamped here");
    assert_eq!(
        runtime.streams.survey::<Capture>(..).budget(4).await.expect("the sealed rows are surveyable").len(),
        1,
        "four points of budget is one bucket over the whole sealed range"
    );

    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn absent_adapter_refuses() {
    let runtime = support::streaming().await;

    assert_eq!(
        runtime.streams.survey::<Capture>(..).budget(64).await.expect_err("the window retains rows, not levels"),
        BeneathWindow::Unserved,
    );
    assert_eq!(runtime.streams.seal::<Capture>().await, Position::ORIGIN, "nothing serving means nothing durable, said plainly");

    runtime.stop().await.unwrap();
}

/* === Tests === */
