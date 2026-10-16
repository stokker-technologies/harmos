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

#[path = "../query/fields.rs"]
mod fields;
#[path = "../query/points.rs"]
mod points;
#[path = "../query/projection.rs"]
mod projection;

/* === Re-Exports === */

/* === Imports === */
use crate::backend::storage_settings;
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, UNIX_EPOCH};
// Third Party Imports

// Local Imports
use super::{Bank, Data, Ledger, Recovery, catalog, history, record, schema};
use harmos::storage::*;
use harmos::{
    BackendFailure, DefinitionId, Error, Identified, Identifier, MemorySize, Result, Runtime,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Document {
    id: u64,
    value: u64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct DocumentV2 {
    id: u64,
    value: u64,
}

struct Fixture {
    storage: StorageManager<Objects>,
    data: Arc<Data<Objects>>,
    runtime: Runtime<Bank>,
}

/* === Enums === */
#[harmos::catalog]
enum Objects {
    Document(Document),
    DocumentV2(DocumentV2),
}

/* === Traits === */

/* === Implementations === */
impl Identified for Document {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("query.document"), 1)
    }
}

impl StoredObject for Document {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(2).unwrap());
    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for DocumentV2 {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("query.document"), 2)
    }
}

impl StoredObject for DocumentV2 {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(2).unwrap());
    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Fixture {
    async fn new() -> Self {
        Self::with_settings(StorageSettings {
            query_page: limits(1, 4096),
            flush_interval: Duration::from_secs(60),
            max_retries: 0,
            ..storage_settings()
        })
        .await
    }
    async fn with_settings(settings: StorageSettings) -> Self {
        let data = Arc::new(Data::<Objects>::with_catalog(history()));
        let storage = StorageManager::new(
            StorageBackends {
                primary: data.clone(),
                state: Arc::new(Recovery::new(history())),
                replicas: vec![],
            },
            settings,
        )
        .unwrap();
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .runtime_with_storage(&storage)
            .await
            .unwrap();
        Self {
            storage,
            data,
            runtime,
        }
    }
    async fn append(&self, value: u64) -> StorageCursor {
        let id = Identifier::new(format!("record-{value}")).unwrap();
        self.storage
            .write_records(&[record(&schema(), &id, value.to_string().as_bytes())])
            .await
            .unwrap()
    }
    async fn finish(self) {
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn limits(rows: usize, bytes: usize) -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(rows).unwrap(),
        bytes: NonZeroUsize::new(bytes).unwrap(),
    }
}

fn decode(bytes: &[u8], _: NonZeroUsize) -> Result<u64> {
    serde_json::from_slice(bytes)
        .map_err(|error| Error::backend("query test decode", BackendFailure::Corrupt, error))
}

async fn until(mut ready: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("page_retains_charge", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    until(|| f.data.control.releases.load(Ordering::SeqCst) == 1).await;
                    let charged = f.storage.status().read_bytes;
                    assert!(charged >= page[0].memory_size());
                    assert!(charged < storage_settings().query_decoded_bytes.get());
                    drop(reader);
                    assert_eq!(f.storage.status().read_bytes, charged);
                    drop(page);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("preserves_page_retry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let oversized = Arc::new(AtomicBool::new(true));
                    let first = oversized.clone();
                    let mut query = RecordQuery::new(schema(), move |_, allowance| {
                        assert!(allowance.get() < 4096);
                        let capacity = if first.swap(false, Ordering::SeqCst) {
                            8192
                        } else {
                            128
                        };
                        let mut value = String::with_capacity(capacity);
                        value.push_str("retained");
                        Ok(value)
                    });
                    query.options.decoded_bytes = NonZeroUsize::new(4096);
                    let mut reader = f.storage.query(query).await.unwrap();
                    let pinned = f.storage.status().read_bytes;
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert_eq!(f.storage.status().read_bytes, pinned);
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].payload, "retained");
                    drop(page);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("capacity_refuses_fetching", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let called = Arc::new(AtomicBool::new(false));
                    let entered = called.clone();
                    let mut query = RecordQuery::new(schema(), move |bytes, allowance| {
                        entered.store(true, Ordering::SeqCst);
                        decode(bytes, allowance)
                    });
                    // A Guaranteed Holder Leaves Less Than A Page Free, So The
                    // Elastic Reader's Page Charge Overfills The Pool.
                    let capacity = storage_settings().budget.read_memory_bytes.get();
                    let holder = f
                        .storage
                        .view_with_limits(QueryLimits {
                            memory_bytes: NonZeroUsize::new(capacity - 128 * 1024).unwrap(),
                            ..storage_settings().query_execution
                        })
                        .await
                        .unwrap();
                    query.options.execution = Some(
                        QueryLimits {
                            memory_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
                            ..storage_settings().query_execution
                        }
                        .elastic(),
                    );
                    let mut reader = f.storage.query(query).await.unwrap();
                    let pinned = f.storage.status().read_bytes;
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert!(!called.load(Ordering::SeqCst));
                    assert_eq!(f.storage.status().read_bytes, pinned);
                    drop((reader, holder));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("page_stays_charged", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let baseline = Arc::new(AtomicUsize::new(0));
                    let failed = Arc::new(AtomicBool::new(false));
                    let storage = f.storage.clone();
                    let charged = baseline.clone();
                    let retry = failed.clone();
                    let mut query = RecordQuery::new(schema(), move |bytes, _| {
                        // Include the output reservation so it cannot mask a lost encoded charge.
                        assert!(
                            storage.status().read_bytes
                                >= charged.load(Ordering::SeqCst) + 512 + 1024 + 4096
                        );
                        if !retry.swap(true, Ordering::SeqCst) {
                            return Err(Error::InvalidInput {
                                field: "query test decode",
                                reason: "retry",
                            });
                        }
                        decode(bytes, NonZeroUsize::new(8).unwrap())
                    });
                    query.options.decoded_bytes = NonZeroUsize::new(512);
                    // Elastic Charges Land In The Pool, Where The Decoder Observes Them.
                    query.options.execution = Some(storage_settings().query_execution.elastic());
                    let mut reader = f.storage.query(query).await.unwrap();
                    baseline.store(f.storage.status().read_bytes, Ordering::SeqCst);

                    assert!(reader.next_page().await.is_err());
                    assert_eq!(
                        f.storage.status().read_bytes,
                        baseline.load(Ordering::SeqCst)
                    );
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].payload, 1);
                    drop(page);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("decoder_panic_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let query = RecordQuery::<u64>::new(schema(), |_, _| panic!("decoder failed"));
                    let mut reader = f.storage.query(query).await.unwrap();

                    let error = tokio::spawn(async move { reader.next_page().await })
                        .await
                        .unwrap_err();
                    assert!(error.is_panic());
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("saved_keeps_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let first = f.append(1).await;
                    let mut other_schema = schema();
                    other_schema.version = std::num::NonZeroU32::new(2).unwrap();
                    f.storage
                        .write_records(&[record(
                            &other_schema,
                            &Identifier::new("other-codec").unwrap(),
                            b"99",
                        )])
                        .await
                        .unwrap();
                    let mut unsaved = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    assert!(unsaved.next_page().await.unwrap().is_none());
                    assert!(f.data.control.writes.lock().unwrap().is_empty());
                    f.storage.flush().await.unwrap();
                    let calls = Arc::new(AtomicUsize::new(0));
                    let captured = calls.clone();
                    let mut result: RecordQueryResult<u64> = f
                        .storage
                        .query(RecordQuery::new(schema(), move |bytes, _| {
                            captured.fetch_add(1, Ordering::SeqCst);
                            decode(bytes, NonZeroUsize::new(8).unwrap())
                        }))
                        .await
                        .unwrap();
                    assert_eq!(calls.load(Ordering::SeqCst), 0);
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page.len(), 1);
                    assert_eq!(page[0].payload, 1);
                    assert_eq!(page[0].position, first);
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert!(result.next_page().await.unwrap().is_none());
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("typed_replay_retries", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    f.append(2).await;
                    let fail = Arc::new(AtomicBool::new(true));
                    let refusal = Arc::clone(&fail);
                    let pointer = Arc::new(AtomicUsize::new(0));
                    let original = Arc::clone(&pointer);
                    let mut query = RecordQuery::new(schema(), move |bytes, allowance| {
                        if refusal.swap(false, Ordering::SeqCst) {
                            original.store(bytes.as_ptr() as usize, Ordering::SeqCst);
                            return Err(Error::InvalidInput {
                                field: "replay decoder",
                                reason: "retry",
                            });
                        }
                        if original.load(Ordering::SeqCst) != 0 {
                            assert_eq!(bytes.as_ptr() as usize, original.swap(0, Ordering::SeqCst));
                        }
                        decode(bytes, allowance)
                    });
                    query.options.page = Some(limits(1, 4096));
                    let mut replay = f.storage.replay(query).await.unwrap();
                    let boundary = replay.boundary().clone();
                    f.append(3).await;
                    assert!(replay.next_page().await.is_err());
                    let charged = f.storage.status().read_bytes;
                    assert!(charged > 0);
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let first = tokio::time::timeout(Duration::from_secs(1), replay.next_page())
                        .await
                        .expect("retry must borrow admitted input without fetching again")
                        .unwrap()
                        .unwrap();
                    assert_eq!(first[0].payload, 1);
                    assert_eq!(replay.boundary(), &boundary);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    let second = replay.next_page().await.unwrap().unwrap();
                    assert_eq!(second[0].payload, 2);
                    assert_eq!(second[0].position, boundary);
                    assert!(replay.next_page().await.unwrap().is_none());
                    assert!(replay.next_page().await.unwrap().is_none());
                    drop((first, second));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("failure_retries_entire", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    static FAIL: AtomicBool = AtomicBool::new(true);
                    fn flaky(bytes: &[u8], _: NonZeroUsize) -> Result<u64> {
                        let value = decode(bytes, NonZeroUsize::new(8).unwrap())?;
                        if value == 2 && FAIL.swap(false, Ordering::SeqCst) {
                            return Err(Error::InvalidInput {
                                field: "test decoder",
                                reason: "retry",
                            });
                        }
                        Ok(value)
                    }
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), flaky);
                    query.options.page = Some(limits(2, 4096));
                    let mut result = f.storage.query(query).await.unwrap();
                    assert!(result.next_page().await.is_err());
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(
                        page.iter().map(|row| row.payload).collect::<Vec<_>>(),
                        [1, 2]
                    );
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_keeps_typed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let first = f.append(1).await;
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    let mut result = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let mut page = Box::pin(result.next_page());
                    tokio::select! {
                        _ = f.data.control.read_entered.notified() => {},
                        result = &mut page => panic!("read did not block: {result:?}"),
                    }
                    drop(page);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].position, first);
                    drop(result);
                    drop(page);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("obey_requested_page_bounds", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    for value in 0..3 {
                        f.append(value).await;
                    }
                    f.storage.flush().await.unwrap();
                    let mut normal = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    assert_eq!(normal.next_page().await.unwrap().unwrap().len(), 1);
                    drop(normal);
                    let mut larger = RecordQuery::new(schema(), decode);
                    larger.options.page = Some(limits(3, 4096));
                    let mut larger = f.storage.query(larger).await.unwrap();
                    assert_eq!(larger.next_page().await.unwrap().unwrap().len(), 3);
                    drop(larger);
                    until(|| f.storage.status().read_bytes == 0).await;
                    for page in [limits(5, 4096), limits(1, 8193)] {
                        let mut query = RecordQuery::new(schema(), decode);
                        query.options.page = Some(page);
                        let mut reader = f.storage.query(query).await.unwrap();
                        let rows = reader.next_page().await.unwrap().unwrap();
                        assert_eq!(rows.len(), page.rows.get().min(3));
                        drop((rows, reader));
                    }
                    let view = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.page = Some(limits(1, 4096));
                    let mut reader = f.storage.query_in(&view, query).unwrap();
                    assert_eq!(
                        reader.next_page().await.unwrap().unwrap()[0].item.as_str(),
                        "record-0"
                    );
                    drop((reader, view));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("failure_never_decode", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    fn forbidden(_: &[u8], _: NonZeroUsize) -> Result<u64> {
                        panic!("an oversized entry must not reach the decoder")
                    }
                    let mut query = RecordQuery::new(schema(), forbidden);
                    query.options.page = Some(limits(1, 1));
                    let mut result = f.storage.query(query).await.unwrap();
                    assert!(result.next_page().await.is_err());
                    assert!(result.next_page().await.is_err());
                    drop(result);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("objects_page_versions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.storage
                        .write_object(&Document { id: 7, value: 0 })
                        .await
                        .unwrap();
                    let first = f
                        .storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    let second = f
                        .storage
                        .write_object(&Document { id: 7, value: 2 })
                        .await
                        .unwrap();
                    f.storage
                        .write_object(&Document { id: 8, value: 3 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    until(|| f.data.objects.lock().unwrap().len() == 3).await;
                    let mut result: ObjectQueryResult<Document> = {
                        let key = 7;
                        let mut query = ObjectQuery::<Document>::new();
                        query.item = Some(&key);
                        query.versions = ObjectVersions::Revisions;
                        f.storage.query(query).await.unwrap()
                    };
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].value, Document { id: 7, value: 1 });
                    assert_eq!(page[0].version, first.version);
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].value.value, 2);
                    assert_eq!(page[0].version, second.version);
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("half_open_exclude", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let stream = Identifier::new("measurements").unwrap();
                    let scope = Identifier::new("run-1").unwrap();
                    for value in 0..6 {
                        let id = Identifier::new(format!("sample-{value}")).unwrap();
                        let payload = value.to_string();
                        let key = schema();
                        let mut record = record(&key, &id, payload.as_bytes());
                        record.source_time =
                            (value != 4).then(|| UNIX_EPOCH + Duration::from_secs(value));
                        record.scope = Some(&scope);
                        f.storage
                            .write_stream(&[StreamWrite {
                                stream: &stream,
                                sequence: 100 + value,
                                record,
                            }])
                            .await
                            .unwrap();
                    }
                    let other_stream = Identifier::new("other-stream").unwrap();
                    let other_scope = Identifier::new("other-scope").unwrap();
                    for (index, (selected_stream, selected_scope)) in
                        [(&other_stream, &scope), (&stream, &other_scope)]
                            .into_iter()
                            .enumerate()
                    {
                        let id = Identifier::new(format!("excluded-{index}")).unwrap();
                        let key = schema();
                        let mut record = record(&key, &id, b"99");
                        record.source_time = Some(UNIX_EPOCH + Duration::from_secs(2));
                        record.scope = Some(selected_scope);
                        f.storage
                            .write_stream(&[StreamWrite {
                                stream: selected_stream,
                                sequence: 200 + index as u64,
                                record,
                            }])
                            .await
                            .unwrap();
                    }
                    f.storage.flush().await.unwrap();
                    let mut query = StreamQuery::new(schema(), decode);
                    query.stream = Some(stream.clone());
                    query.scope = Some(scope);
                    query.source_time = Some(SourceTimeRange {
                        start: UNIX_EPOCH + Duration::from_secs(1),
                        end: UNIX_EPOCH + Duration::from_secs(3),
                    });
                    let mut result: StreamQueryResult<u64> = f.storage.query(query).await.unwrap();
                    for value in [1, 2] {
                        let page = result.next_page().await.unwrap().unwrap();
                        assert_eq!(page[0].record.payload, value);
                        assert_eq!(page[0].sequence, 100 + value);
                        assert_eq!(page[0].stream, stream);
                    }
                    assert!(result.next_page().await.unwrap().is_none());
                    let mut invalid = StreamQuery::new(schema(), decode);
                    invalid.source_time = Some(SourceTimeRange {
                        start: UNIX_EPOCH,
                        end: UNIX_EPOCH,
                    });
                    assert!(f.storage.query(invalid).await.is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("ranges_preserve_descending", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let first = f.append(1).await;
                    f.append(2).await;
                    let third = f.append(3).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.after = first.sequence;
                    query.options.through = Some(third.sequence);
                    query.options.order = ReadOrder::Descending;
                    let mut result = f.storage.query(query).await.unwrap();
                    assert_eq!(result.next_page().await.unwrap().unwrap()[0].payload, 3);
                    assert_eq!(result.next_page().await.unwrap().unwrap()[0].payload, 2);
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("queries_share_saved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    let file = f
                        .storage
                        .write_file(
                            FileMetadata {
                                id: Identifier::new("report").unwrap(),
                                scope: None,
                                media_type: "text/plain".into(),
                                expected_length: Some(3),
                                expected_sha256: None,
                            },
                            b"abc",
                        )
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let mut files: FileQueryResult =
                        f.storage.query(FileQuery::new()).await.unwrap();
                    let view = files.view().unwrap().clone();
                    let saved = view.saved().unwrap();
                    let mut records = f
                        .storage
                        .query_in(&view, RecordQuery::new(schema(), decode))
                        .unwrap();
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    assert_eq!(records.view().unwrap().saved().unwrap(), saved);
                    drop(view);
                    assert_eq!(files.next_page().await.unwrap().unwrap()[0], file);
                    assert_eq!(records.next_page().await.unwrap().unwrap()[0].payload, 1);
                    assert!(records.next_page().await.unwrap().is_none());
                    assert!(files.next_page().await.unwrap().is_none());
                    assert!(files.view().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("foreign_views_expired", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let other = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let view = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    assert!(
                        other
                            .storage
                            .query_in(&view, RecordQuery::new(schema(), decode))
                            .is_err()
                    );
                    let mut result = f
                        .storage
                        .query_in(&view, RecordQuery::new(schema(), decode))
                        .unwrap();
                    f.storage.close().await.unwrap();
                    assert!(matches!(result.next_page().await, Err(Error::ViewExpired)));
                    drop(result);
                    drop(view);
                    f.finish().await;
                    other.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("shared_budget_refuses_oversized_decode", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.decoded_bytes =
                        NonZeroUsize::new(storage_settings().budget.read_memory_bytes.get() + 1);
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let before = f.storage.status().read_bytes;
                    assert!(matches!(
                        f.storage.query(query).await,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, before);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("shared_budget_refuses_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.execution = Some(QueryLimits {
                        memory_bytes: NonZeroUsize::new(
                            storage_settings().budget.read_memory_bytes.get() + 1,
                        )
                        .unwrap(),
                        ..storage_settings().query_execution
                    });
                    assert!(matches!(
                        f.storage.query(query).await,
                        Err(Error::InvalidInput {
                            field: "read view",
                            ..
                        })
                    ));
                    assert!(f.data.control.view_ids.lock().unwrap().is_empty());
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("backend_refuses_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.execution = Some(QueryLimits {
                        memory_bytes: NonZeroUsize::new(1).unwrap(),
                        ..storage_settings().query_execution
                    });
                    assert!(f.storage.query(query).await.is_err());
                    until(|| f.storage.status().read_bytes == 0).await;
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("but_not_idle", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.execution = Some(QueryLimits {
                        timeout: Duration::from_secs(2),
                        ..storage_settings().query_execution
                    });
                    let mut result = f.storage.query(query).await.unwrap();
                    // Idle is below the separate view idle limit, but exceeds compute timeout.
                    tokio::time::advance(Duration::from_secs(3)).await;
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let mut page = Box::pin(result.next_page());
                        tokio::select! {
                            _ = &mut page => panic!("blocked read unexpectedly completed"),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                        tokio::time::advance(Duration::from_secs(1)).await;
                    }
                    // Cancellation leaves the page and pin intact, with one second consumed.
                    assert!(f.storage.status().read_bytes > 0);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert_eq!(result.next_page().await.unwrap().unwrap()[0].payload, 1);
                    drop(result);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("budget_keeps_pin", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.execution = Some(QueryLimits {
                        timeout: Duration::from_secs(1),
                        ..storage_settings().query_execution
                    });
                    let mut result = f.storage.query(query).await.unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let mut page = Box::pin(result.next_page());
                        tokio::select! {
                            _ = &mut page => panic!("blocked read unexpectedly completed"),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                        tokio::time::advance(Duration::from_secs(2)).await;
                        assert!(matches!(
                            page.await,
                            Err(Error::ResourceExhausted {
                                resource: "query computation time"
                            })
                        ));
                    }
                    assert!(f.storage.status().read_bytes > 0);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert!(matches!(
                        result.next_page().await,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    drop(result);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("public_query_dispatch", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let execution = storage_settings().query_execution;
                    let different = f
                        .storage
                        .view_with_limits(QueryLimits {
                            timeout: execution.timeout / 2,
                            ..execution
                        })
                        .await
                        .unwrap();
                    assert!(
                        <RecordQuery<u64> as StorageQuery<()>>::open(
                            RecordQuery::new(schema(), decode),
                            different,
                            limits(1, 4096),
                            execution.timeout,
                        )
                        .is_err()
                    );
                    let bounded = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    for (page, timeout) in [
                        (limits(4, 8192), Duration::from_secs(5)),
                        (limits(1, 4096), Duration::from_secs(6)),
                    ] {
                        assert!(
                            <RecordQuery<u64> as StorageQuery<()>>::open(
                                RecordQuery::new(schema(), decode),
                                bounded.clone(),
                                page,
                                timeout,
                            )
                            .is_err()
                        );
                    }
                    drop(bounded);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("orders_refuse_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::Aggregate;
                    use harmos::storage::frame::{
                        Expr, FrameQuery, GroupBy, ScalarField, ScalarPath,
                    };
                    use harmos::storage::reduce::{Agg, ScalarKind};

                    let f = Fixture::new().await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    f.data
                        .control
                        .frame_capture_only
                        .store(true, Ordering::SeqCst);
                    let mut selection = ReadSelection::new(ReadKind::Stream);
                    selection.schema = Some(schema());
                    let mut frame = Frame::new(NonZeroUsize::MIN, NonZeroUsize::MIN);
                    let scan = frame.scan(
                        selection,
                        vec![
                            ScalarField {
                                path: ScalarPath::Stream,
                                kind: ScalarKind::Text,
                            },
                            ScalarField {
                                path: ScalarPath::SourceTime,
                                kind: ScalarKind::Datetime,
                            },
                        ],
                    );
                    let shift = frame.shift(scan, vec![0], 1);
                    frame.group_by(
                        GroupBy::new(scan).agg(Expr::diff(shift), Agg::Of(Aggregate::Mean)),
                    );
                    let query = FrameQuery {
                        frame,
                        options: QueryOptions::default(),
                    };
                    let read_bytes = f.storage.status().read_bytes;
                    let result =
                        tokio::time::timeout(Duration::from_secs(1), f.storage.query(query))
                            .await
                            .unwrap();
                    assert!(matches!(
                        result,
                        Err(Error::InvalidInput {
                            field: "storage frame",
                            ..
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, read_bytes);
                    assert!(f.data.control.view_ids.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("objects_keep_fixed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    for (id, value) in [(7, 0), (8, 1), (7, 2)] {
                        f.storage
                            .write_object(&Document { id, value })
                            .await
                            .unwrap();
                    }
                    f.storage.flush().await.unwrap();
                    let mut query = ObjectQuery::<Document>::new();
                    query.options.order = ReadOrder::Descending;
                    let mut result = f.storage.query(query).await.unwrap();
                    f.storage
                        .write_object(&Document { id: 8, value: 3 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let first = result.next_page().await.unwrap().unwrap();
                    assert_eq!(first[0].value, Document { id: 7, value: 2 });
                    let second = result.next_page().await.unwrap().unwrap();
                    assert_eq!(second[0].value, Document { id: 8, value: 1 });
                    assert!(result.next_page().await.unwrap().is_none());
                    let mut query = ObjectQuery::<Document>::new();
                    query.options.after = first[0].position.sequence;
                    let mut result = f.storage.query(query).await.unwrap();
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].value, Document { id: 8, value: 3 });
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("latest_reduces_codec", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage
                        .write_object(&DocumentV2 { id: 7, value: 2 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let mut older = f
                        .storage
                        .query(ObjectQuery::<Document>::new())
                        .await
                        .unwrap();
                    assert!(older.next_page().await.unwrap().is_none());
                    let mut current = f
                        .storage
                        .query(ObjectQuery::<DocumentV2>::new())
                        .await
                        .unwrap();
                    let page = current.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].value.value, 2);
                    assert!(current.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("latest_refuses_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.data
                        .control
                        .refuse_latest_objects
                        .store(true, Ordering::SeqCst);
                    let before = f.data.control.query_views.load(Ordering::SeqCst);
                    assert!(matches!(
                        f.storage.query(ObjectQuery::<Document>::new()).await,
                        Err(Error::InvalidInput {
                            field: "object query",
                            ..
                        })
                    ));
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    let mut revisions = ObjectQuery::<Document>::new();
                    revisions.versions = ObjectVersions::Revisions;
                    let mut result = f.storage.query(revisions).await.unwrap();
                    assert!(result.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("browse_retains_computation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.data
                        .control
                        .allow_empty_browse
                        .store(true, Ordering::SeqCst);
                    let query = ObjectQuery::<Document>::new()
                        .project(vec![Identifier::new("value").unwrap()])
                        .unwrap();
                    let mut reader = f.storage.query(query).await.unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    assert!(reader.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("browse_refuses_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let before = f.data.control.query_views.load(Ordering::SeqCst);
                    let query = ObjectQuery::<Document>::new()
                        .project(vec![Identifier::new("value").unwrap()])
                        .unwrap();
                    assert!(matches!(
                        f.storage.query(query).await,
                        Err(Error::InvalidInput {
                            field: "object browse",
                            ..
                        })
                    ));
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("object_preflight_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.storage
                        .write_object(&Document { id: 7, value: 41 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    {
                        let mut objects = f.data.objects.lock().unwrap();
                        objects.values_mut().next().unwrap().value.payload = vec![0xff];
                    }
                    let calls = Arc::new(AtomicUsize::new(0));
                    let estimates = calls.clone();
                    let query = ObjectQuery::<Document>::new().with_decoding_memory(move |bytes| {
                        assert_eq!(bytes, &[0xff]);
                        estimates.fetch_add(1, Ordering::SeqCst);
                        Ok(usize::MAX)
                    });
                    let mut reader = f.storage.query(query).await.unwrap();
                    for expected in 1..=2 {
                        assert!(matches!(
                            reader.next_page().await,
                            Err(Error::ResourceExhausted {
                                resource: "query decoded memory"
                            })
                        ));
                        assert_eq!(calls.load(Ordering::SeqCst), expected);
                    }
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    drop(reader);
                    let mut native = f
                        .storage
                        .query(ObjectQuery::<Document>::new())
                        .await
                        .unwrap();
                    assert!(native.next_page().await.is_err());
                    drop(native);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("refusal_preserves_page", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let document = Document { id: 7, value: 41 };
                    let payload = harmos::Message::encode_to_vec(&document);
                    f.storage.write_object(&document).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let calls = Arc::new(AtomicUsize::new(0));
                    let estimates = calls.clone();
                    let mut query =
                        ObjectQuery::<Document>::new().with_decoding_memory(move |bytes| {
                            assert_eq!(bytes, payload);
                            if estimates.fetch_add(1, Ordering::SeqCst) == 0 {
                                Ok(usize::MAX)
                            } else {
                                Ok(std::mem::size_of::<Document>())
                            }
                        });
                    query.options.decoded_bytes = NonZeroUsize::new(4096);
                    let mut reader = f.storage.query(query).await.unwrap();
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    let pinned = f.storage.status().read_bytes;
                    assert!(pinned > 0);
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(calls.load(Ordering::SeqCst), 2);
                    assert_eq!(page[0].value, document);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    assert!(reader.next_page().await.unwrap().is_none());
                    drop((page, reader));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("query_keeps_view", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(9).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    // Elastic Charges Land In The Pool, Where The Projection Observes Them.
                    query.options.execution = Some(storage_settings().query_execution.elastic());
                    let mut reader = f.storage.query(query).await.unwrap();
                    reader.retain_view().unwrap();
                    let saved = reader.view().unwrap().saved().unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let input = page
                        .try_into_value(NonZeroUsize::new(64).unwrap(), |records, _| {
                            Ok(records[0].payload)
                        })
                        .unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    assert_eq!(reader.view().unwrap().saved().unwrap(), saved);
                    let retained = f.storage.status().read_bytes;
                    let failure = reader.project(
                        &input,
                        NonZeroUsize::new(64).unwrap(),
                        |value, _| -> Result<u64> {
                            assert_eq!(*value, 9);
                            assert!(f.storage.status().read_bytes >= retained + 64);
                            Err(Error::InvalidInput {
                                field: "fixture finalization",
                                reason: "retry",
                            })
                        },
                    );
                    assert!(failure.is_err());
                    assert_eq!(*input, 9);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = reader.project(
                            &input,
                            NonZeroUsize::new(64).unwrap(),
                            |_, _| -> Result<u64> { panic!("fixture projection panic") },
                        );
                    }));
                    assert!(panic.is_err());
                    assert_eq!(*input, 9);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let output = reader
                        .project(&input, NonZeroUsize::new(64).unwrap(), |value, _| {
                            Ok(*value + 1)
                        })
                        .unwrap();
                    assert_eq!(*output, 10);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    drop((output, input, reader));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
}

#[tokio::test]
async fn cancelled_admission_keeps_typed_cursor() {
    let f = Fixture::new().await;
    let position = f.append(7).await;
    f.storage.flush().await.unwrap();
    let mut query = RecordQuery::new(schema(), decode);
    query.options.execution = Some(storage_settings().query_execution.elastic());
    let mut reader = f.storage.query(query).await.unwrap();
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let capacity = storage_settings().budget.read_memory_bytes.get();
    let external = f
        .storage
        .reserve_arrow_memory(capacity - occupied - overhead)
        .unwrap();
    let mut next = Box::pin(reader.next_page());
    assert!(matches!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(
            next.as_mut(),
            cx
        )))
        .await,
        std::task::Poll::Pending
    ));
    drop(next);
    drop(external);

    // Cancellation Before Fetch Must Leave The Original Typed Cursor Intact.
    let page = reader.next_page().await.unwrap().unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].position, position);
    drop(page);
    assert!(reader.next_page().await.unwrap().is_none());
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn typed_output_admission_reloads_without_advancing_cursor() {
    let f = Fixture::new().await;
    let position = f.append(7).await;
    f.storage.flush().await.unwrap();
    let mut query = RecordQuery::new(schema(), decode);
    query.options.execution = Some(storage_settings().query_execution.elastic());
    query.options.decoded_bytes = NonZeroUsize::new(128 * 1024);
    let mut reader = f.storage.query(query).await.unwrap();
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let capacity = storage_settings().budget.read_memory_bytes.get();
    // Raw Fetch Fits, But Its Decoded Output Must Wait For Another Owner.
    let external = f
        .storage
        .reserve_arrow_memory(capacity - occupied - overhead - 64 * 1024)
        .unwrap();
    let mut next = Box::pin(reader.next_page());
    assert!(matches!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(
            next.as_mut(),
            cx
        )))
        .await,
        std::task::Poll::Pending
    ));
    drop(external);
    let page = next.await.unwrap().unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].position, position);
    drop(page);
    assert!(reader.next_page().await.unwrap().is_none());
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn typed_projection_waits_before_invoking_callback() {
    let f = Fixture::new().await;
    let position = f.append(7).await;
    f.storage.flush().await.unwrap();
    let mut query = RecordQuery::new(schema(), decode);
    query.options.execution = Some(storage_settings().query_execution.elastic());
    query.options.decoded_bytes = NonZeroUsize::new(16 * 1024);
    let mut reader = f.storage.query(query).await.unwrap();
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let capacity = storage_settings().budget.read_memory_bytes.get();
    // The Decoded Page Fits; Its Projection Waits Without Running User Code.
    let external = f
        .storage
        .reserve_arrow_memory(capacity - occupied - overhead - 64 * 1024)
        .unwrap();
    let calls = std::cell::Cell::new(0usize);
    let mut next = Box::pin(reader.next_page_projected(
        NonZeroUsize::new(128 * 1024).unwrap(),
        |rows, _| {
            calls.set(calls.get() + 1);
            assert_eq!(rows.len(), 1);
            Ok(rows[0].position.clone())
        },
    ));
    assert!(matches!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(
            next.as_mut(),
            cx
        )))
        .await,
        std::task::Poll::Pending
    ));
    assert_eq!(calls.get(), 0);
    drop(external);
    let page = next.await.unwrap().unwrap();
    assert_eq!(*page, position);
    assert_eq!(calls.get(), 1);
    drop(page);
    assert!(reader.next_page().await.unwrap().is_none());
    drop(reader);
    f.finish().await;
}
