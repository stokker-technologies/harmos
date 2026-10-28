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
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use tracing::instrument::WithSubscriber;
use tracing::{Instrument, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

// Local Imports
use super::{Fixture, config, record, schema, write};
use harmos_storage::Identifier;
use harmos_storage::StreamInput;
use harmos_storage::schema::Cardinality;
use harmos_storage::schema::graph::{Field, Kind};
use harmos_storage::state::StateBackend;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Captured>>, Arc<tokio::sync::Notify>);

#[derive(Clone, Debug, Default)]
struct Captured {
    spans: BTreeMap<u64, CapturedSpan>,
    active: BTreeMap<u64, u64>,
    links: Vec<(u64, u64)>,
    created: u64,
    closed: usize,
    unresolved_links: usize,
}

#[derive(Clone, Debug)]
struct CapturedSpan {
    name: &'static str,
    parent: Option<u64>,
    closed: Option<usize>,
    fields: BTreeMap<String, String>,
}

struct Fields<'a>(&'a mut BTreeMap<String, String>);

/* === Enums === */

enum FlushGate {
    Write,
    Receipt,
}

/* === Traits === */

/* === Implementations === */

impl tracing::field::Visit for Fields<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(&self, attributes: &tracing::span::Attributes<'_>, id: &tracing::Id, context: Context<'_, S>) {
        let parent = context.span(id).unwrap().parent().map(|parent| parent.id().into_u64());
        let mut fields = BTreeMap::new();
        attributes.record(&mut Fields(&mut fields));
        let mut capture = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let parent = parent.and_then(|parent| capture.active.get(&parent).copied());
        let span = CapturedSpan {
            name: attributes.metadata().name(),
            parent,
            closed: None,
            fields,
        };
        capture.created += 1;
        let created = capture.created;
        capture.active.insert(id.into_u64(), created);
        capture.spans.insert(created, span);
    }

    fn on_record(&self, id: &tracing::Id, values: &tracing::span::Record<'_>, _: Context<'_, S>) {
        let mut capture = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let created = capture.active.get(&id.into_u64()).copied();
        if let Some(span) = created.and_then(|created| capture.spans.get_mut(&created)) {
            values.record(&mut Fields(&mut span.fields));
        }
    }

    fn on_follows_from(&self, id: &tracing::Id, follows: &tracing::Id, _: Context<'_, S>) {
        let mut capture = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let target = capture.active.get(&id.into_u64()).copied();
        let source = capture.active.get(&follows.into_u64()).copied();
        if let (Some(target), Some(source)) = (target, source) {
            capture.links.push((target, source));
        } else {
            capture.unresolved_links += 1;
        }
    }

    fn on_close(&self, id: tracing::Id, _: Context<'_, S>) {
        let mut capture = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        capture.closed += 1;
        let closed = capture.closed;
        let created = capture.active.remove(&id.into_u64());
        if let Some(span) = created.and_then(|created| capture.spans.get_mut(&created)) {
            span.closed = Some(closed);
        }
        drop(capture);
        self.1.notify_one();
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test(start_paused = true)]
async fn storage_trace_handoffs() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_flush.store(true, Ordering::SeqCst);
    let capture = Capture::default();
    let subscriber = Registry::default().with(capture.clone());
    let dispatch = tracing::Dispatch::new(subscriber);

    for number in 1..=2 {
        let request = tracing::dispatcher::with_default(&dispatch, || tracing::debug_span!("storage_test_request", number));
        write(&f.storage, "private-item").instrument(request).with_subscriber(dispatch.clone()).await;
    }
    let second = f.storage.cursor_accepted();
    {
        let request = tracing::dispatcher::with_default(&dispatch, || tracing::debug_span!("cancelled_request"));
        let mut cancelled = Box::pin(write(&f.storage, "private-cancelled").instrument(request).with_subscriber(dispatch.clone()));
        tokio::select! {
            biased;
            _ = &mut cancelled => panic!("durability is blocked"),
            _ = f.data.control.flush_entered.notified() => {}
        }
        tracing::dispatcher::with_default(&dispatch, || drop(cancelled));
    }
    {
        let captured = capture.0.lock().unwrap().clone();
        let cancelled = captured.spans.values().find(|span| span.name == "cancelled_request").unwrap();
        assert!(cancelled.closed.is_some(), "cancelled request retained: {captured:#?}");
        let backend = captured.spans.values().find(|span| span.name == "storage.backend").unwrap();
        assert!(backend.closed.is_none(), "physical work remains active after caller cancellation");
    }
    assert_eq!(f.storage.cursor_accepted(), second);
    f.data.control.block_flush.store(false, Ordering::SeqCst);
    f.data.control.flush_release.notify_one();
    f.storage.await_durable(&second).await.unwrap();

    let selection = harmos_storage::query::RecordSelection {
        schema: schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    };
    let request = tracing::dispatcher::with_default(&dispatch, || tracing::debug_span!("reader_request"));
    let mut reader = f
        .storage
        .follow::<harmos_storage::StoredRecord>(None, selection.clone())
        .instrument(request)
        .with_subscriber(dispatch.clone())
        .await
        .unwrap();
    let page = reader.next().await.unwrap();
    assert_eq!(page.entries().len(), 2);
    drop(page);
    drop(reader);

    let mut cancelled_reader =
        f.storage.follow::<harmos_storage::StoredRecord>(None, selection).with_subscriber(dispatch.clone()).await.unwrap();
    f.data.control.block_read.store(true, Ordering::SeqCst);
    let mut cancelled_page = Box::pin(cancelled_reader.next());
    tokio::select! {
        biased;
        _ = &mut cancelled_page => panic!("backend read is blocked"),
        _ = f.data.control.read_entered.notified() => {}
    }
    drop(cancelled_page);
    drop(cancelled_reader);
    f.data.control.block_read.store(false, Ordering::SeqCst);
    let history = f.storage.cursor_saved().history;
    f.finish().await;

    let directory = tempfile::tempdir().unwrap();
    let backend = harmos_storage::state::file::FileBackend::new(directory.path().to_owned(), history, super::fixture::state_settings());
    let position = backend.open().with_subscriber(dispatch.clone()).await.unwrap();
    backend.read_snapshot(NonZeroUsize::new(4096).unwrap()).with_subscriber(dispatch.clone()).await.unwrap();
    backend.flush(&position).with_subscriber(dispatch.clone()).await.unwrap();
    backend.close().with_subscriber(dispatch.clone()).await.unwrap();

    let captured = capture.0.lock().unwrap().clone();
    let (&batch_id, batch) = captured.spans.iter().find(|(_, span)| span.name == "storage.batching.write").unwrap();
    assert!(batch.parent.is_none());
    assert!(captured.links.iter().all(|(target, _)| *target != batch_id));
    let contributions: Vec<_> = captured.spans.iter().filter(|(_, span)| span.name == "storage.batch.contribution").collect();
    assert_eq!(contributions.len(), 2);
    for (&contribution_id, contribution) in contributions {
        for field in ["store", "epoch", "domain", "first_index", "last_index", "through"] {
            assert_eq!(contribution.fields[field], batch.fields[field]);
        }
        let sources: Vec<_> = captured.links.iter().filter(|(target, _)| *target == contribution_id).map(|(_, source)| *source).collect();
        assert_eq!(sources.len(), 1);
        let queued = &captured.spans[&sources[0]];
        assert_eq!(queued.name, "storage.queue");
        assert!(queued.parent.is_some());
        assert!(queued.closed.unwrap() < contribution.closed.unwrap());
        assert_eq!(contribution.fields["outcome"], "\"completed\"");
    }
    let page = captured.spans.values().find(|span| span.name == "storage.reader.page").unwrap();
    let origin = &captured.spans[&page.parent.unwrap()];
    assert_eq!(origin.name, "storage.follow.follow");
    assert!(page.closed.is_some());
    assert!(captured.spans.values().filter(|span| span.name == "storage.reader.page").all(|span| span.closed.is_some()));
    assert!(
        captured
            .spans
            .values()
            .any(|span| { span.name == "storage.backend" && span.fields.get("outcome").is_some_and(|outcome| outcome == "\"cancelled\"") })
    );
    let backend = captured
        .spans
        .values()
        .find(|span| span.name == "storage.backend" && span.fields.get("method").is_some_and(|method| method.contains("primary.write")))
        .unwrap();
    assert_eq!(backend.fields["outcome"], "\"completed\"");
    let direct: Vec<_> = captured.spans.values().filter(|span| span.name == "storage.state.backend").collect();
    assert_eq!(direct.len(), 4);
    assert!(direct.iter().all(|span| span.closed.is_some() && span.fields["outcome"] == "\"completed\""));
    assert!(captured.spans.values().any(|span| span.name == "storage.backend.blocking" && span.closed.is_some()));
    assert!(captured.spans.values().all(|span| !span.fields.contains_key("payload") && !span.fields.contains_key("item")));
    assert!(captured.spans.values().flat_map(|span| span.fields.values()).all(|value| !value.contains("private-")));
}

#[tokio::test(start_paused = true)]
async fn filtered_storage_unchanged() {
    let capture = Capture::default();
    let subscriber = Registry::default().with(capture.clone()).with(tracing_subscriber::filter::LevelFilter::OFF);
    let dispatch = tracing::Dispatch::new(subscriber);
    async {
        let f = Fixture::new(config()).await;
        let accepted = write(&f.storage, "private-item").await;
        assert_eq!(f.storage.flush().await.unwrap(), accepted);
        assert_eq!(f.data.records.lock().unwrap().len(), 1);
        f.finish().await;
    }
    .with_subscriber(dispatch)
    .await;
    assert!(capture.0.lock().unwrap().spans.is_empty());
}

#[tokio::test(start_paused = true)]
async fn split_collectors_coalesce() {
    let f = Fixture::new(config()).await;
    let first_capture = Capture::default();
    let second_capture = Capture::default();
    let first_dispatch = tracing::Dispatch::new(Registry::default().with(first_capture.clone()));
    let second_dispatch = tracing::Dispatch::new(Registry::default().with(second_capture.clone()));
    write(&f.storage, "first-private").with_subscriber(first_dispatch.clone()).await;
    let accepted = write(&f.storage, "second-private").with_subscriber(second_dispatch.clone()).await;
    f.data.control.failures.store(1, Ordering::SeqCst);
    assert!(f.storage.flush().await.is_err());
    assert_eq!(f.storage.flush().await.unwrap(), accepted);
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [2, 2]);

    f.storage.write_recovery(vec![b"private-recovery-first".to_vec()]).with_subscriber(first_dispatch.clone()).await.unwrap();
    f.storage.write_recovery(vec![b"private-recovery-second".to_vec()]).with_subscriber(second_dispatch.clone()).await.unwrap();
    assert_eq!(f.storage.flush_recovery().await.unwrap().sequence, 2);
    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), [2]);

    let mut files = Vec::new();
    for (id, dispatch) in [("first-file", &first_dispatch), ("second-file", &second_dispatch)] {
        let metadata = harmos_storage::FileMetadata {
            id: Identifier::new(id).unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        };
        files.push(f.storage.produce_file(metadata).with_subscriber(dispatch.clone()).await.unwrap());
    }
    let save = tracing::dispatcher::with_default(&first_dispatch, || f.storage.capture_files(None)).unwrap();
    f.data.control.block_checkpoint.store(true, Ordering::SeqCst);
    let mut persisting = Box::pin(f.storage.persist_files(&save));
    tokio::select! {
        biased;
        _ = &mut persisting => panic!("checkpoint is blocked"),
        _ = f.data.control.checkpoint_entered.notified() => {}
    }
    for capture in [&first_capture, &second_capture] {
        let captured = capture.0.lock().unwrap().clone();
        let contribution = captured.spans.values().find(|span| span.name == "storage.file.checkpoint.contribution").unwrap();
        assert!(contribution.closed.is_none());
    }
    f.data.control.block_checkpoint.store(false, Ordering::SeqCst);
    f.data.control.checkpoint_release.notify_one();
    persisting.await.unwrap();
    assert_eq!(f.data.saved_productions.lock().unwrap().len(), 2);
    drop(save);
    drop(files);
    f.finish().await;

    for capture in [first_capture, second_capture] {
        let captured = capture.0.lock().unwrap().clone();
        assert_eq!(captured.unresolved_links, 0, "foreign span ID crossed a collector boundary");
        let contributions: Vec<_> = captured.spans.values().filter(|span| span.name == "storage.batch.contribution").collect();
        assert_eq!(contributions.len(), 3);
        for contribution in &contributions {
            let (first, last) = match contribution.fields["domain"].as_str() {
                "\"data\"" => ("1", "2"),
                "\"recovery\"" => ("3", "4"),
                domain => panic!("unexpected batch domain: {domain}"),
            };
            assert_eq!(contribution.fields["first_index"], first);
            assert_eq!(contribution.fields["last_index"], last);
            assert_eq!(contribution.fields["through"], "2");
            assert_eq!(contribution.fields["request_domain"], contribution.fields["domain"]);
        }
        assert_eq!(contributions.iter().filter(|span| span.fields["domain"] == "\"recovery\"").count(), 1);
        assert!(contributions.iter().any(|span| span.fields["outcome"] == "\"failed\""));
        assert!(contributions.iter().any(|span| span.fields["outcome"] == "\"completed\""));
        assert!(captured.spans.values().any(|span| span.name == "storage.file.checkpoint.contribution"));
        assert!(captured.spans.values().all(|span| span.closed.is_some()), "collector retained spans: {captured:#?}");
        for (_, source) in &captured.links {
            assert!(matches!(captured.spans[source].name, "storage.queue" | "storage.file.queue" | "storage.file.checkpoint.queue"));
        }
        assert!(captured.spans.values().flat_map(|span| span.fields.values()).all(|value| !value.contains("private")));
    }
}

#[tokio::test(start_paused = true)]
async fn typed_prefix_survives() {
    let f = Fixture::new(config()).await;
    f.storage.write_schema(schema(), harmos_storage::schema::graph::Schema { root: 0, definitions: vec![vec![]] }).await.unwrap();
    write(&f.storage, "one").await;
    write(&f.storage, "two").await;
    for sequence in 1..=2 {
        f.storage
            .write_stream(vec![StreamInput {
                stream: Identifier::new("voltage").unwrap().clone(),
                sequence,
                record: record(&schema(), &Identifier::new(format!("row-{sequence}")).unwrap(), &[1]),
            }])
            .await
            .unwrap();
    }
    f.storage.write_schema(schema(), harmos_storage::schema::graph::Schema { root: 0, definitions: vec![vec![]] }).await.unwrap();
    write(&f.storage, "three").await;
    write(&f.storage, "four").await;
    f.storage.write_recovery(vec![b"first".to_vec()]).await.unwrap();
    f.storage.write_recovery(vec![b"second".to_vec()]).await.unwrap();
    // The First Type Is Durable Even When The Following Typed Call Fails.
    f.data.control.write_failure_at.store(2, Ordering::SeqCst);
    assert!(f.storage.flush().await.is_err());
    assert_eq!(f.storage.cursor_saved().sequence, 1);
    assert_eq!(f.data.schemas.lock().unwrap().len(), 1);
    assert!(f.data.records.lock().unwrap().is_empty());
    assert!(f.data.rows.lock().unwrap().is_empty());
    assert_eq!(f.storage.flush().await.unwrap().sequence, 8);
    f.storage.flush_recovery().await.unwrap();
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), vec![1, 2, 2, 2, 1, 2]);
    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 2, 3, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), vec![2]);

    let schemas = f.data.schemas.lock().unwrap().clone();
    assert_eq!(schemas.keys().copied().collect::<Vec<_>>(), [1, 6]);

    let records = f.data.records.lock().unwrap().clone();
    assert_eq!(records.keys().copied().collect::<Vec<_>>(), [2, 3, 7, 8]);
    assert!(records.values().all(|record| record.payload == [1, 2, 3]));

    let rows = f.data.rows.lock().unwrap().clone();
    assert_eq!(rows.keys().copied().collect::<Vec<_>>(), [4, 5]);
    for sequence in 1..=2 {
        let row = &rows[&(sequence + 3)];
        assert_eq!(row.sequence, sequence);
        assert_eq!(row.record.payload, [1]);
    }

    let recovery = f.recovery.entries.lock().unwrap().clone();
    assert_eq!(recovery.keys().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(recovery[&1].payload, b"first");
    assert_eq!(recovery[&2].payload, b"second");

    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn flush_excludes_later() {
    let cases = [
        (FlushGate::Write, &["first", "second", "third"][..], &["fourth", "fifth"][..], &[1, 2, 3][..], &[3, 2][..], &[3, 5][..]),
        (FlushGate::Receipt, &["one", "two"][..], &["later"][..], &[1, 2][..], &[2, 1][..], &[2, 3][..]),
    ];
    for (gate, before, after, prefix, batches, receipts) in cases {
        let f = Fixture::new(config()).await;
        let (blocked, entered, release) = match gate {
            FlushGate::Write => (&f.data.control.block_write, &f.data.control.write_entered, &f.data.control.write_release),
            FlushGate::Receipt => (&f.data.control.block_flush, &f.data.control.flush_entered, &f.data.control.flush_release),
        };
        blocked.store(true, Ordering::SeqCst);
        tokio::task::yield_now().await;
        for item in before {
            write(&f.storage, item).await;
        }
        let storage = f.storage.clone();
        let flush = storage.flush();
        tokio::pin!(flush);
        tokio::select! {
            biased;
            _ = &mut flush => panic!("the backend stage is blocked"),
            _ = entered.notified() => {}
        }
        for item in after {
            write(&f.storage, item).await;
        }
        blocked.store(false, Ordering::SeqCst);
        release.notify_one();
        assert_eq!(flush.await.unwrap().sequence, receipts[0]);

        // Completing A Barrier Cannot Start A Later Write.
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), prefix);
        assert_eq!(f.data.control.write_sizes.lock().unwrap().as_slice(), &batches[..1]);
        assert_eq!(f.storage.status().pending_batches, after.len());

        f.storage.flush().await.unwrap();
        assert_eq!(f.data.control.write_sizes.lock().unwrap().as_slice(), batches);
        assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), receipts);
        f.finish().await;
    }
}

#[tokio::test]
async fn cancelled_admission_unqueued() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_flush.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let second = write(&f.storage, "two").await;
    {
        let blocked = write(&f.storage, "cancelled");
        tokio::pin!(blocked);
        tokio::select! {
            _ = &mut blocked => panic!("the saved receipt is still blocked"),
            _ = f.data.control.flush_entered.notified() => {}
        }
    }
    assert_eq!(f.storage.status().accepted.sequence, 2);
    assert_eq!(f.storage.status().saved.sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 2);
    f.data.control.block_flush.store(false, Ordering::SeqCst);
    f.data.control.flush_release.notify_one();
    f.storage.await_durable(&second).await.unwrap();
    assert_eq!(write(&f.storage, "later").await.sequence, 3);
    assert_eq!(f.storage.status().saved.sequence, 2);
    assert_eq!(*f.data.control.flushes.lock().unwrap(), [2]);
    f.finish().await;
}

#[tokio::test]
async fn pressure_preserves_flush() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let storage = f.storage.clone();
    let flush = storage.flush();
    tokio::pin!(flush);
    tokio::select! { biased; _ = &mut flush => panic!("write is blocked"), _ = f.data.control.write_entered.notified() => {} }
    write(&f.storage, "two").await;
    let blocked = write(&storage, "three");
    tokio::pin!(blocked);
    assert!(super::pending(blocked.as_mut()).await, "queue is full");
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    let (saved, admitted) = tokio::join!(flush, blocked);
    assert_eq!(saved.unwrap().sequence, 1);
    assert_eq!(admitted.sequence, 3);
    assert_eq!(*f.data.control.flushes.lock().unwrap(), [1]);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn failure_retains_work() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.flush_failures.store(1, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let key = schema();
    let item = Identifier::new("not-admitted").unwrap();
    let inputs = [record(&key, &item, &[1])];
    let failure = f.storage.write_records((inputs).to_vec()).await.unwrap_err();
    assert!(matches!(failure, harmos_storage::Error::Persistence { .. }));
    assert_eq!(f.storage.status().accepted.sequence, 1);
    assert_eq!(f.storage.status().saved.sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 1);
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().saved.sequence, 1);
    assert_eq!(f.storage.status().pending_batches, 0);
    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 1]);
    assert_eq!(f.data.records.lock().unwrap().len(), 1);
    assert_eq!(*f.data.control.flushes.lock().unwrap(), [1, 1]);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn reservation_never_flush() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
    let f = Fixture::new(policy).await;
    write(&f.storage, "one").await;
    let snapshot = f.storage.read_snapshot(NonZeroUsize::new(7000).unwrap()).await.unwrap();
    assert!(snapshot.is_none());
    assert_eq!(f.storage.status().saved.sequence, 0);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn queued_writes_wait() {
    let f = Fixture::new(config()).await;
    tokio::task::yield_now().await;
    for item in ["one", "two", "three", "four"] {
        write(&f.storage, item).await;
        tokio::task::yield_now().await;
    }
    for entry in [b"first".as_slice(), b"second".as_slice()] {
        f.storage.write_recovery(vec![entry.to_vec()]).await.unwrap();
        tokio::task::yield_now().await;
    }
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    assert!(f.recovery.control.writes.lock().unwrap().is_empty());
    assert_eq!(f.storage.status().accepted.sequence, 4);
    assert_eq!(f.storage.status().recovery_accepted.sequence, 2);
    assert_eq!(f.storage.status().pending_batches, 6);

    // The Private Periodic Save Interval Is 100 Milliseconds.
    tokio::time::advance(Duration::from_millis(99)).await;
    tokio::task::yield_now().await;
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    assert!(f.recovery.control.writes.lock().unwrap().is_empty());

    f.data.control.block_flush.store(true, Ordering::SeqCst);
    f.recovery.control.block_flush.store(true, Ordering::SeqCst);
    tokio::time::advance(Duration::from_millis(1)).await;
    // Independent State Work May Reach Its Flush Before Data.
    tokio::join!(
        async {
            f.data.control.flush_entered.notified().await;
            f.data.control.block_flush.store(false, Ordering::SeqCst);
            f.data.control.flush_release.notify_one();
        },
        async {
            f.recovery.control.flush_entered.notified().await;
            f.recovery.control.block_flush.store(false, Ordering::SeqCst);
            f.recovery.control.flush_release.notify_one();
        }
    );
    f.storage.flush_recovery().await.unwrap();
    f.storage.flush().await.unwrap();
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [4]);
    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), [2]);
    assert_eq!(f.storage.status().saved.sequence, 4);
    assert_eq!(f.storage.status().recovery_saved.sequence, 2);
    f.finish().await;
}

#[tokio::test]
async fn progress_preserves_cursor() {
    let f = Fixture::new(config()).await;
    write(&f.storage, "before").await;
    let snapshot = harmos_storage::state::RecoverySnapshot {
        required_data: f.storage.cursor_saved(),
        through: f.storage.status().recovery_accepted,
        payload: vec![7],
    };
    f.storage.write_snapshot(snapshot.clone()).await.unwrap();
    write(&f.storage, "after").await;
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().saved.sequence, 2);
    assert_eq!(f.storage.status().pending_batches, 1);
    assert!(f.recovery.snapshot.lock().unwrap().is_none());
    f.storage.flush_recovery().await.unwrap();
    assert_eq!(*f.recovery.snapshot.lock().unwrap(), Some(snapshot));
    assert_eq!(*f.recovery.control.flushes.lock().unwrap(), [0]);
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn typed_batch_retries() {
    let f = Fixture::new(config()).await;
    let key = schema();
    let item = Identifier::new("group").unwrap();
    let payload = vec![7; 100];
    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
    f.data.control.block_write.store(true, Ordering::SeqCst);
    f.data.control.failures.store(1, Ordering::SeqCst);
    let accepted = f.storage.write_records((records).to_vec()).await.unwrap();

    // A Typed Batch Keeps Its Original Data While The Backend Is Blocked.
    let flushing = {
        let storage = f.storage.clone();
        tokio::spawn(async move { storage.flush().await })
    };
    f.data.control.write_entered.notified().await;
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    assert!(flushing.await.unwrap().is_err());

    // A Failed Attempt Leaves The Whole Batch Available At Its Original Cursor.
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 1);
    assert_eq!(f.storage.flush_until(&accepted).await.unwrap(), accepted);
    assert_eq!(f.data.records.lock().unwrap().len(), 8);
    assert_eq!(f.data.control.write_sizes.lock().unwrap().as_slice(), &[8, 8]);
    assert!(f.data.control.write_sizes.lock().unwrap().iter().all(|size| *size == 8));
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn item_refuses_whole() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(4096).unwrap();
    let f = Fixture::new(policy).await;
    let key = schema();
    let item = Identifier::new("oversized").unwrap();
    let large = vec![255; 8192];
    let inputs = [record(&key, &item, b"small"), record(&key, &item, &large)];
    assert!(matches!(f.storage.write_records((inputs).to_vec()).await, Err(harmos_storage::Error::ResourceExhausted { .. })));
    assert_eq!(f.storage.cursor_accepted().sequence, 0);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn schema_decoding_bounded() {
    let fields: Vec<_> = (1..=64)
        .map(|tag| Field {
            name: Cow::Owned(format!("field-{tag}")),
            tag,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        })
        .collect();
    let graph = Arc::new(harmos_storage::schema::graph::Schema { root: 0, definitions: vec![fields] });
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(16 * 1024).unwrap();
    let f = Fixture::new(policy.clone()).await;
    let key = schema();
    let position = f.storage.write_schema(key.clone(), graph.clone()).await.unwrap();
    assert_eq!(position.sequence, 1);
    assert!(f.storage.status().pending_bytes < policy.request.max_pending_write_bytes.get());
    f.storage.flush().await.unwrap();
    {
        let saved = f.data.schemas.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[&1].schema, graph);
        assert!(Arc::ptr_eq(&saved[&1].schema, &graph));
        for definitions in [1usize, 32, 256] {
            let mut metadata = saved[&1].clone();
            let fields = (1..definitions)
                .map(|definition| Field {
                    name: Cow::Owned(format!("child_{definition}")),
                    tag: definition as u32,
                    cardinality: Cardinality::Repeated,
                    kind: Kind::Message { definition },
                })
                .collect();
            let graph = Arc::make_mut(&mut metadata.schema);
            graph.definitions = vec![Vec::new(); definitions];
            graph.definitions[0] = fields;
            let encoded = serde_json::to_vec(&metadata).unwrap();
            let decoded: harmos_storage::StorageSchema = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, metadata);
        }
    }
    f.finish().await;
}

/* === Tests === */

#[tokio::test]
async fn raw_reads_retained() {
    let f = Fixture::new(config()).await;
    let through = f.storage.write_recovery(vec![vec![7; 256]]).await.unwrap();
    f.storage.flush_recovery().await.unwrap();
    let after = harmos_storage::RecoveryPosition {
        history: through.history.clone(),
        sequence: 0,
    };
    let page = f
        .storage
        .read_recovery(
            &after,
            &through,
            harmos_storage::ReadLimits {
                rows: NonZeroUsize::MIN,
                bytes: NonZeroUsize::new(4096).unwrap(),
            },
        )
        .await
        .unwrap();
    let alias = page.clone();
    let pointer = page.entries[0].payload.as_ptr();
    let owner = alias.owner();
    drop(page);
    drop(alias);
    let retained: &harmos_storage::state::RecoveryPage = std::borrow::Borrow::borrow(owner.as_ref());
    assert_eq!(retained.entries[0].payload.as_ptr(), pointer);
    drop(owner);

    f.storage
        .write_snapshot(harmos_storage::state::RecoverySnapshot {
            required_data: f.storage.cursor_saved(),
            through,
            payload: vec![8; 256],
        })
        .await
        .unwrap();
    f.storage.flush_recovery().await.unwrap();
    let snapshot = f.storage.read_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap().unwrap();
    let alias = snapshot.clone();
    drop(snapshot);
    drop(alias);
    f.finish().await;
}

#[tokio::test]
async fn snapshot_payload_limit_survives_reopen() {
    use harmos_domain::state::Position;
    use harmos_storage::state::file::FileBackend;
    use harmos_storage::{ReadLimits, StorageBackends, StorageManager};

    let maximum = NonZeroUsize::new(4096).unwrap();
    let limits = ReadLimits { rows: NonZeroUsize::MIN, bytes: maximum };
    for length in [maximum.get() - 2, maximum.get() - 1, maximum.get()] {
        let directory = tempfile::tempdir().unwrap();
        let data = super::backend::Data::new(super::backend::history());
        let data = Arc::new(data);
        let recovery = FileBackend::new(directory.path().to_owned(), super::backend::history(), super::fixture::state_settings());
        let recovery = Arc::new(recovery);
        let backends = || StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        };
        let storage: StorageManager = StorageManager::new(backends(), config()).unwrap();

        storage.acquire().await.unwrap();
        let reader = storage.recovery_reader(limits, Some(maximum)).await.unwrap();
        let _durable = reader.finish(Position::ORIGIN);
        super::fixture::start(&storage).await;

        let mut permit = storage.reserve_snapshot(maximum.get()).await.unwrap();
        let error = permit.snapshot_payload(vec![7; maximum.get() + 1]).unwrap_err();
        assert!(matches!(
            error,
            harmos_storage::Error::InvalidInput {
                field: "state snapshot",
                reason: "payload exceeds reservation",
            }
        ));
        drop(permit);
        assert_eq!(storage.status().pending_bytes, 0);
        assert!(storage.read_snapshot(NonZeroUsize::new(8192).unwrap()).await.unwrap().is_none());

        let payload = vec![7; length];
        let mut permit = storage.reserve_snapshot(maximum.get()).await.unwrap();
        permit.snapshot_payload(payload.clone()).unwrap();
        permit.commit(Position::ORIGIN);
        storage.flush_recovery().await.unwrap();
        let written = storage.read_snapshot(NonZeroUsize::new(8192).unwrap()).await.unwrap().unwrap();
        assert!(serde_json::to_vec(&*written).unwrap().len() > maximum.get());
        drop(written);
        storage.close().await.unwrap();

        let reopened: StorageManager = StorageManager::new(backends(), config()).unwrap();
        reopened.acquire().await.unwrap();
        let reader = reopened.recovery_reader(limits, Some(maximum)).await.unwrap();
        let snapshot = reader.snapshot().await.unwrap().unwrap();
        assert_eq!(snapshot.payload, payload);
        drop(snapshot);
        drop(reader);
        reopened.close().await.unwrap();
    }
}

#[tokio::test]
async fn snapshot_recovery_enforces_payload_limit() {
    use harmos_storage::{ReadLimits, StorageBackends, StorageManager};

    let f = Fixture::new(config()).await;
    let maximum = NonZeroUsize::new(4096).unwrap();
    f.storage
        .write_snapshot(harmos_storage::state::RecoverySnapshot {
            required_data: f.storage.cursor_saved(),
            through: f.storage.status().recovery_saved,
            payload: vec![7; maximum.get() + 1],
        })
        .await
        .unwrap();
    f.storage.flush_recovery().await.unwrap();
    let backends = StorageBackends {
        primary: f.data.clone(),
        state: f.recovery.clone(),
    };
    f.finish().await;

    let reopened: StorageManager = StorageManager::new(backends, config()).unwrap();
    reopened.acquire().await.unwrap();
    let limits = ReadLimits { rows: NonZeroUsize::MIN, bytes: maximum };
    let reader = reopened.recovery_reader(limits, Some(maximum)).await.unwrap();
    let error = reader.snapshot().await.unwrap_err();
    assert!(matches!(
        error,
        harmos_storage::Error::InvalidInput {
            field: "state snapshot",
            reason: "invalid managed snapshot boundary or size",
        }
    ));
    drop(reader);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn recovery_owner_retained() {
    let f = Fixture::new(config()).await;
    let mut payload = Vec::with_capacity(4096);
    payload.extend_from_slice(b"recovery");
    let pointer = payload.as_ptr() as usize;
    f.recovery.control.retain_recovery.store(true, Ordering::SeqCst);
    f.storage.write_recovery(vec![payload]).await.unwrap();
    f.storage.flush_recovery().await.unwrap();
    assert_eq!(f.storage.status().pending_batches, 0);
    let detached = f.recovery.control.retained_recovery.lock().unwrap().take().unwrap();
    assert_eq!(detached.as_slice()[0].payload.as_ptr() as usize, pointer);
    let empty = detached.slice(1..1);
    drop(detached);
    drop(empty);
    assert_eq!(f.storage.status().pending_bytes, 0);

    let mut payload = Vec::with_capacity(2048);
    payload.extend_from_slice(b"snapshot");
    let pointer = payload.as_ptr() as usize;
    f.recovery.control.retain_snapshot.store(true, Ordering::SeqCst);
    let snapshot = harmos_storage::state::RecoverySnapshot {
        required_data: f.storage.cursor_saved(),
        through: f.storage.status().recovery_saved,
        payload,
    };
    f.storage.write_snapshot(snapshot).await.unwrap();
    f.storage.flush_recovery().await.unwrap();
    let detached = f.recovery.control.retained_snapshot.lock().unwrap().take().unwrap();
    assert_eq!(detached.as_slice()[0].payload.as_ptr() as usize, pointer);
    drop(detached);
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[harmos_domain::message]
struct TracedRow {
    value: String,
}

#[tokio::test(start_paused = true)]
async fn retained_observer_traced() {
    let f = Fixture::new(config()).await;
    let capture = Capture::default();
    let dispatch = tracing::Dispatch::new(Registry::default().with(capture.clone()));
    async {
        let request = tracing::debug_span!("retained_request");
        async {
            let key = schema();
            let graph = harmos_storage::schema::graph::Schema::from_declaration(
                <TracedRow as harmos_storage::schema::Described>::DEFINITION,
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            f.storage.write_schema(key.clone(), graph).await.unwrap();
            let value = TracedRow { value: "private-projection".into() };
            let payload = harmos_storage::Message::encode(&value);
            f.storage.write_records(vec![record(&key, &Identifier::new("trace-row").unwrap(), &payload)]).await.unwrap();
            let saved = f.storage.flush().await.unwrap();
            let session = f.storage.clone();
            let mut reader = session
                .read_records::<TracedRow>(harmos_storage::RecordRead {
                    at: saved.clone(),
                    selection: harmos_storage::query::RecordSelection {
                        schema: key,
                        scope: None,
                        item: None,
                        source_start: None,
                        source_end: None,
                    },
                    after: 0,
                })
                .await
                .unwrap();
            let page = reader.next().await.unwrap().unwrap();
            let projected = page.try_map(|mut rows| Ok::<_, harmos_storage::Error>(rows.pop().unwrap().payload.value)).unwrap();
            let peer = projected.clone();
            drop(reader);
            drop(session);
            f.data.snapshots_released().await;
            let mut observer = Box::pin(f.storage.wait_saved_after(&saved));
            assert!(super::fixture::pending(observer.as_mut()).await);
            {
                let captured = capture.0.lock().unwrap();
                let observer = captured.spans.values().find(|span| span.name == "storage.manager.status.wait_saved_after").unwrap();
                assert!(observer.closed.is_none());
                let projection = captured.spans.values().find(|span| span.name == "storage.read_batch.try_map").unwrap();
                assert!(projection.closed.is_some());
                assert_eq!(projection.fields["outcome"], "\"completed\"");
            }
            drop(observer);
            let older = harmos_storage::StorageCursor { sequence: 0, ..saved.clone() };
            assert_eq!(f.storage.wait_saved_after(&older).await.unwrap(), saved);
            assert_eq!(projected.as_str(), "private-projection");
            drop(projected);
            drop(peer);
        }
        .instrument(request)
        .await;
    }
    .with_subscriber(dispatch)
    .await;
    f.finish().await;
    let captured = capture.0.lock().unwrap();
    let observers: Vec<_> = captured.spans.values().filter(|span| span.name == "storage.manager.status.wait_saved_after").collect();
    assert_eq!(observers.len(), 2);
    assert!(observers.iter().any(|span| span.fields["outcome"] == "\"cancelled\""));
    assert!(observers.iter().any(|span| span.fields["outcome"] == "\"completed\""));
    assert!(captured.spans.values().all(|span| span.closed.is_some()));
    assert!(captured.spans.values().any(|span| span.name == "storage.manager.read_records" && span.parent.is_some()));
    assert!(captured.spans.values().flat_map(|span| span.fields.values()).all(|value| !value.contains("private-")));
    assert!(captured.spans.values().all(|span| !span.fields.contains_key("payload") && !span.fields.contains_key("path")));
}

#[tokio::test]
async fn filesystem_trace_lifetimes() {
    use harmos_domain::model::storage::files::{ByteRange, FileSource};
    use harmos_domain::model::storage::{DirectoryRule, DirectorySelection};
    use harmos_storage::FileRead;
    use harmos_storage::{JsonStorage, JsonStorageSettings, StorageBackend};
    use std::future::Future;
    use std::task::Poll;

    let f = Fixture::new(config()).await;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("private-file.bin");
    std::fs::write(&path, b"private-payload").unwrap();
    let capture = Capture::default();
    let dispatch = tracing::Dispatch::new(Registry::default().with(capture.clone()));
    let request = tracing::dispatcher::with_default(&dispatch, || tracing::debug_span!("filesystem_request"));
    let range = ByteRange { offset: 0, limit: None };
    let mut reader = f
        .storage
        .read_file(FileRead {
            source: FileSource::Local(path.clone()),
            range,
        })
        .instrument(request.clone())
        .with_subscriber(dispatch.clone())
        .await
        .unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), b"private-payload");
    assert!(reader.next().await.unwrap().is_none());
    drop(reader);

    let selection = DirectorySelection {
        root: root.path().to_owned(),
        recursive: false,
        rules: vec![DirectoryRule { pattern: "**".into(), include: true }],
    };
    let mut listing =
        f.storage.list_directory(selection.clone()).instrument(request.clone()).with_subscriber(dispatch.clone()).await.unwrap();
    assert!(listing.next().await.unwrap().is_some());
    while listing.next().await.unwrap().is_some() {}
    drop(listing);
    let mut watcher =
        f.storage.watch_directory(selection, false).instrument(request.clone()).with_subscriber(dispatch.clone()).await.unwrap();
    assert!(watcher.next().await.unwrap().is_some());
    let mut pending = Box::pin(watcher.next());
    std::future::poll_fn(|task| {
        assert!(pending.as_mut().poll(task).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    drop(watcher);

    let digest = f
        .storage
        .hash_file(FileRead { source: FileSource::Local(path), range })
        .instrument(request.clone())
        .with_subscriber(dispatch.clone())
        .await
        .unwrap();
    assert_eq!(digest.bytes_read, 15);
    let backend_root = tempfile::tempdir().unwrap();
    let backend = JsonStorage::new(
        JsonStorageSettings {
            root: backend_root.path().to_owned(),
            max_entry_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
        super::history(),
    );
    async {
        assert!(backend.cursor_saved().await.is_err());
        backend.open().await.unwrap();
        backend.cursor_saved().await.unwrap();
        backend.close().await.unwrap();
    }
    .instrument(request.clone())
    .with_subscriber(dispatch.clone())
    .await;
    drop(backend);
    tracing::dispatcher::with_default(&dispatch, || drop(request));
    f.finish().await;

    // Native Watcher Drop Signals Shutdown; Its Callback Can Retire On Another Thread.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let changed = capture.1.notified();
            if capture.0.lock().unwrap().spans.values().all(|span| span.closed.is_some()) {
                break;
            }
            changed.await;
        }
    })
    .await
    .expect("native watcher and request spans must close after shutdown");

    let captured = capture.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for name in [
        "storage.local.open",
        "storage.local.next",
        "storage.file.hash_file",
        "storage.directory.page",
        "storage.directory.watch.page",
        "storage.directory.subscription",
        "storage.backend.blocking",
        "storage.json.backend",
    ] {
        assert!(captured.spans.values().any(|span| span.name == name && span.parent.is_some()), "missing {name}");
    }
    assert!(captured.spans.values().all(|span| span.closed.is_some()), "retained spans: {captured:#?}");
    assert!(captured.spans.values().any(|span| span.name == "storage.directory.watch.page" && span.fields["outcome"] == "\"cancelled\""));
    assert!(captured.spans.values().any(|span| span.name == "storage.json.backend" && span.fields["outcome"] == "\"failed\""));
    assert!(captured.spans.values().any(|span| span.name == "storage.backend.blocking" && span.fields["outcome"] == "\"completed\""));
    assert!(captured.spans.values().any(|span| span.name == "storage.backend.blocking" && span.fields["outcome"] == "\"failed\""));
    assert!(captured.spans.values().all(|span| !span.fields.contains_key("payload") && !span.fields.contains_key("path")));
    assert!(captured.spans.values().flat_map(|span| span.fields.values()).all(|value| !value.contains("private-")));
}
