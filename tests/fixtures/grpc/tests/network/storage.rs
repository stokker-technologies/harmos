/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../crates/harmos/tests/support/storage.rs"]
#[allow(
    dead_code,
    reason = "shared backend provides controls for the full storage suite"
)]
mod backend;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports
use arrow_array::{Array, StringArray, StructArray, TimestampNanosecondArray, UInt64Array};
use harmos::storage::arrow::ArrowBatch;
use harmos::storage::query::{
    Query, QueryCompletion, QueryOutput, RecordSelection, RowSlice, Source, StreamSelection,
};
use prost::Message as _;

// Local Imports
use harmos::storage::operations::Followed;
use harmos::storage::{
    FileMetadata, FileReference, StorageBackends, StorageCursor, StorageManager,
};
use harmos::{Identifier, Runtime};
use harmos_grpc_fixture::storage::v1::{
    storage_client::StorageClient, storage_follow_output, storage_follow_stream_output,
    storage_messages, storage_query_output,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
struct Reading {
    value: String,
}

#[harmos::message]
struct StoredDocument {
    id: String,
    text: String,
}

/* === Enums === */
#[harmos::catalog]
enum StoredObjects {
    Document(StoredDocument),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Identified for StoredDocument {
    fn definition() -> (harmos::DefinitionId, u32) {
        (harmos::DefinitionId::new("test.document"), 1)
    }
}
impl harmos::storage::StoredObject for StoredDocument {
    type Key = String;
    const RETENTION: harmos::storage::ObjectRetention = harmos::storage::ObjectRetention::Latest;
    fn key(&self) -> &String {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn followed(encoded: &[u8]) -> harmos::storage::StoredRecord {
    match <Followed as harmos::Protobuf>::decode(encoded).unwrap() {
        Followed::Record(record) => record,
        Followed::Boundaries(_) => panic!("boundaries are reported once, before records"),
    }
}

async fn query_batches(
    client: &mut StorageClient<tonic::transport::Channel>,
    request: Query,
) -> Vec<ArrowBatch> {
    let encoded = harmos::Protobuf::encode_to_vec(&request);
    let wire = storage_messages::query::Row1::decode(encoded.as_slice()).unwrap();
    let mut response = client.query(wire).await.unwrap().into_inner();
    let mut schema = None;
    let mut batches = Vec::new();
    loop {
        match response
            .message()
            .await
            .unwrap_or_else(|error| panic!("query {:?} failed: {error}", request.source))
            .expect("query must terminate explicitly")
            .value
            .unwrap()
        {
            storage_query_output::Value::Item(item) => {
                let output =
                    <QueryOutput as harmos::Protobuf>::decode(&item.encode_to_vec()).unwrap();
                match harmos::storage::query::QueryArrow::try_from(&output).unwrap() {
                    harmos::storage::query::QueryArrow::Schema(header) => {
                        assert!(schema.is_none() && batches.is_empty());
                        schema = Some(header);
                    }
                    harmos::storage::query::QueryArrow::Batch(batch) => {
                        assert_eq!(
                            batch.as_record_batch().schema().as_ref(),
                            schema
                                .as_ref()
                                .expect("schema precedes batches")
                                .as_schema()
                        );
                        batches.push(batch);
                    }
                }
            }
            storage_query_output::Value::Response(done) => {
                let done =
                    <QueryCompletion as harmos::Protobuf>::decode(&done.encode_to_vec()).unwrap();
                assert_eq!(done.at, request.at);
                assert_eq!(
                    done.rows,
                    batches
                        .iter()
                        .map(|batch| batch.as_record_batch().num_rows() as u64)
                        .sum::<u64>()
                );
                assert!(schema.is_some());
                assert!(response.message().await.unwrap().is_none());
                return batches;
            }
        }
    }
}

fn write_batch(
    schema: harmos::storage::SchemaKey,
    destination: &str,
    metadata: StructArray,
    payload: StructArray,
) -> harmos::storage::writes::DataBatch {
    use arrow_array::{ArrayRef, RecordBatch};
    use arrow_schema::{Field, Schema};
    let fields = vec![
        Field::new("_storage", metadata.data_type().clone(), false),
        Field::new("payload", payload.data_type().clone(), false),
    ];
    let columns: Vec<ArrayRef> = vec![Arc::new(metadata), Arc::new(payload)];
    harmos::storage::writes::DataBatch {
        sequence: 1,
        destination: Identifier::new(destination).unwrap(),
        schema,
        data: ArrowBatch::try_new(
            RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap(),
        )
        .unwrap()
        .into(),
    }
}

async fn write_completion<T: prost::Message + Default>(
    mut response: tonic::Streaming<T>,
) -> StorageCursor {
    use harmos::grpc::OutputEnvelope;
    use harmos::storage::writes::{WriteCompletion, WriteReceipt};
    let mut accepted = None;
    let mut durable = None;
    loop {
        let frame = response
            .message()
            .await
            .unwrap()
            .expect("write must terminate explicitly");
        let envelope = <OutputEnvelope<WriteReceipt, WriteCompletion> as harmos::Protobuf>::decode(
            &frame.encode_to_vec(),
        )
        .unwrap();
        match envelope {
            OutputEnvelope::Item(WriteReceipt::Accepted(receipt)) => {
                assert!(accepted.is_none());
                assert_eq!(receipt.sequence, 1);
                accepted = Some(receipt);
            }
            OutputEnvelope::Item(WriteReceipt::Durable(receipt)) => {
                assert!(durable.is_none());
                assert_eq!(accepted.as_ref(), Some(&receipt));
                durable = Some(receipt);
            }
            OutputEnvelope::Response(done) => {
                assert_eq!(done.batches, 1);
                assert_eq!(
                    done.cursor,
                    durable.expect("durability precedes completion").cursor
                );
                assert!(response.message().await.unwrap().is_none());
                return done.cursor;
            }
        }
    }
}

/* === Tests === */
#[tokio::test]
async fn storage_uses_data() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let runtime = Runtime::<super::App>::builder((), ())
            .register(super::Lanes::catalog())
            .runtime()
            .await
            .unwrap();
        let data = Arc::new(backend::Data::<StoredObjects>::with_catalog(
            backend::history(),
        ));
        let storage = Arc::new(
            StorageManager::<StoredObjects>::new(
                StorageBackends {
                    primary: data.clone(),
                    state: Arc::new(backend::Recovery::new(backend::history())),
                },
                backend::storage_settings(),
            )
            .unwrap(),
        );
        storage.start(&runtime).await.unwrap();
        harmos::storage::operations::register(
            &runtime.operations,
            "storage",
            super::Lanes::Open,
            storage.clone(),
        )
        .unwrap();
        let file = storage
            .write_file(
                FileMetadata {
                    id: Identifier::new("recording").unwrap(),
                    scope: None,
                    media_type: "application/octet-stream".into(),
                    expected_length: None,
                    expected_sha256: None,
                },
                b"recorded bytes",
            )
            .await
            .unwrap();
        let key = harmos::storage::SchemaKey {
            payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
            id: Identifier::new("reading").unwrap(),
            version: NonZeroU32::MIN,
        };
        let graph = harmos::schema::graph::Schema::from_declaration(
            <Reading as harmos::schema::Described>::DEFINITION,
            &harmos::config::SchemaSettings::default(),
        )
        .unwrap();
        storage.write_schema(&key, &graph).await.unwrap();
        let native_payload = harmos::Message::encode_to_vec(&Reading {
            value: "native bytes".into(),
        });
        let live_payload = harmos::Message::encode_to_vec(&Reading {
            value: "live bytes".into(),
        });
        let live_row_payload = harmos::Message::encode_to_vec(&Reading {
            value: "live row".into(),
        });
        let sample = Identifier::new("sample").unwrap();
        let source = Identifier::new("sensor").unwrap();
        // Use A Pre-Epoch Tick Representable On Linux And Windows
        let source_time = std::time::UNIX_EPOCH - Duration::from_nanos(100);
        assert_eq!(
            source_time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_err()
                .duration(),
            Duration::from_nanos(100)
        );
        let record = harmos::storage::RecordWrite {
            schema: &key,
            scope: None,
            item: &sample,
            source: Some(&source),
            source_time: Some(source_time),
            payload: &native_payload,
        };
        storage.write_records(&[record]).await.unwrap();
        storage
            .write_stream(&[harmos::storage::StreamWrite {
                stream: &source,
                sequence: 123,
                record,
            }])
            .await
            .unwrap();
        let through = storage.cursor_accepted();
        // Arrow Frames Retain Schema, Arrays And IPC Bytes Together.
        // This Round Trip Uses The Same Capacity As The Typed Write Stories.
        let capacity = harmos::operations::StreamSettings {
            items: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        };
        let service = harmos_grpc_fixture::StorageServer::new(
            harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
            harmos::grpc::server::Settings {
                calls: NonZeroUsize::new(4).unwrap(),
                decode: harmos::grpc::DecodeSettings {
                    message_bytes: capacity.bytes,
                    retained_bytes: backend::storage_settings().budget.read_memory_bytes,
                },
                encode: harmos::grpc::EncodeSettings {
                    message_bytes: capacity.bytes,
                    buffer_bytes: NonZeroUsize::new(256).unwrap(),
                },
                input: capacity,
                output: capacity,
                opening_timeout: Duration::from_secs(1),
            },
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let incoming = futures_util::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(socket, _)| socket), listener))
        });
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                }),
        );
        let mut client = StorageClient::connect(format!("http://{address}"))
            .await
            .unwrap();
        let saved = storage.flush_until(&through).await.unwrap();
        assert_eq!(saved, storage.cursor_saved());
        let files = query_batches(
            &mut client,
            Query {
                at: saved.clone(),
                source: Source::Files(harmos::storage::query::FileSelection {
                    scope: None,
                    item: None,
                }),
                transforms: vec![],
                slice: RowSlice::default(),
            },
        )
        .await;
        assert_eq!(files.len(), 1);
        let files = files[0].as_record_batch();
        assert_eq!(
            files
                .column_by_name("item")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            file.metadata.id.as_str()
        );
        assert_eq!(
            files
                .column_by_name("length")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(0),
            file.length
        );
        let selection = StreamSelection {
            schema: key.clone(),
            scope: None,
            item: None,
            stream: None,
            source_start: Some(std::time::UNIX_EPOCH - Duration::from_secs(1)),
            source_end: Some(std::time::UNIX_EPOCH),
            after: 0,
        };
        // Finished File Bytes Use The Same Operation Until Explicit Completion.
        let request = harmos::storage::files::ReadFile {
            file: file.clone(),
            range: harmos::storage::files::ByteRange {
                offset: 2,
                limit: Some(7),
            },
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::read_file::Row1::decode(encoded.as_slice()).unwrap();
        let mut content = client.read_file(wire).await.unwrap().into_inner();
        let mut bytes = Vec::new();
        loop {
            use harmos_grpc_fixture::storage::v1::storage_read_file_output;
            match content
                .message()
                .await
                .unwrap()
                .expect("file read must terminate explicitly")
                .value
                .unwrap()
            {
                storage_read_file_output::Value::Item(item) => {
                    let batch = <harmos::storage::files::FileBatch as harmos::Protobuf>::decode(
                        &item.encode_to_vec(),
                    )
                    .unwrap();
                    assert_eq!(batch.offset, 2 + bytes.len() as u64);
                    bytes.extend_from_slice(&batch.bytes);
                }
                storage_read_file_output::Value::Response(done) => {
                    let done =
                        <harmos::storage::files::FileCompletion as harmos::Protobuf>::decode(
                            &done.encode_to_vec(),
                        )
                        .unwrap();
                    assert_eq!(done.bytes, bytes.len() as u64);
                    break;
                }
            }
        }
        assert_eq!(bytes, b"corded ");
        assert!(content.message().await.unwrap().is_none());
        let records = query_batches(
            &mut client,
            Query {
                at: saved.clone(),
                source: Source::Records(RecordSelection {
                    schema: key.clone(),
                    scope: None,
                    item: Some(sample.clone()),
                    source_start: selection.source_start,
                    source_end: selection.source_end,
                }),
                transforms: vec![],
                slice: RowSlice::default(),
            },
        )
        .await;
        assert_eq!(records.len(), 1);
        let record_batch = records[0].as_record_batch();
        assert_eq!(record_batch.num_rows(), 1);
        let metadata = record_batch
            .column_by_name("_storage")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(
            metadata
                .column_by_name("item")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            sample.as_str()
        );
        assert_eq!(
            metadata
                .column_by_name("source")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            source.as_str()
        );
        assert_eq!(
            metadata
                .column_by_name("source_time")
                .unwrap()
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap()
                .value(0),
            -100
        );
        let payload = record_batch
            .column_by_name("payload")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(
            payload
                .column_by_name("value")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "native bytes"
        );
        let rows = query_batches(
            &mut client,
            Query {
                at: saved.clone(),
                source: Source::Stream(StreamSelection {
                    schema: key.clone(),
                    scope: None,
                    item: Some(sample.clone()),
                    stream: Some(source.clone()),
                    source_start: selection.source_start,
                    source_end: selection.source_end,
                    after: 0,
                }),
                transforms: vec![],
                slice: RowSlice::default(),
            },
        )
        .await;
        assert_eq!(rows.len(), 1);
        let row_batch = rows[0].as_record_batch();
        assert_eq!(row_batch.num_rows(), 1);
        let metadata = row_batch
            .column_by_name("_storage")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(
            metadata
                .column_by_name("stream")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            source.as_str()
        );
        assert_eq!(
            metadata
                .column_by_name("sequence")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(0),
            123
        );
        assert_eq!(
            metadata
                .column_by_name("source_time")
                .unwrap()
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap()
                .value(0),
            -100
        );
        let payload = row_batch
            .column_by_name("payload")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(
            payload
                .column_by_name("value")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "native bytes"
        );
        let encoded = harmos::Protobuf::encode_to_vec(&harmos::storage::follow::Records {
            after: None,
            selection: RecordSelection {
                schema: selection.schema.clone(),
                scope: None,
                item: None,
                source_start: selection.source_start,
                source_end: selection.source_end,
            },
        });
        let query = storage_messages::follow::Row1::decode(encoded.as_slice()).unwrap();
        let mut following = client.follow(query).await.unwrap().into_inner();
        let storage_follow_output::Value::Item(boundaries) =
            following.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected starting boundaries");
        };
        let Followed::Boundaries(boundaries) =
            <Followed as harmos::Protobuf>::decode(&boundaries.encode_to_vec()).unwrap()
        else {
            panic!("boundaries are reported once, before records");
        };
        assert_eq!(boundaries.saved, saved);
        let storage_follow_output::Value::Item(first) =
            following.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected historical record");
        };
        let historical = followed(&first.encode_to_vec());
        assert_eq!(historical.schema, key);
        assert_eq!(historical.item, sample);
        assert_eq!(historical.source, Some(source.clone()));
        assert_eq!(historical.source_time, Some(source_time));
        assert_eq!(historical.payload, native_payload);
        let live = Identifier::new("live").unwrap();
        let position = storage
            .write_records(&[harmos::storage::RecordWrite {
                schema: &key,
                scope: None,
                item: &live,
                source: Some(&source),
                source_time: Some(source_time),
                payload: &live_payload,
            }])
            .await
            .unwrap();
        let storage_follow_output::Value::Item(next) =
            following.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected live record");
        };
        let next = followed(&next.encode_to_vec());
        assert_eq!(next.item, live);
        assert_eq!(next.position, position);
        drop(following);
        let encoded = harmos::Protobuf::encode_to_vec(&harmos::storage::follow::Stream {
            after: None,
            selection: selection.clone(),
        });
        let query = storage_messages::follow_stream::Row1::decode(encoded.as_slice()).unwrap();
        let mut following_rows = client.follow_stream(query).await.unwrap().into_inner();
        let storage_follow_stream_output::Value::Item(first) = following_rows
            .message()
            .await
            .unwrap()
            .unwrap()
            .value
            .unwrap()
        else {
            panic!("expected historical row");
        };
        let first =
            <harmos::storage::StoredRow as harmos::Protobuf>::decode(&first.encode_to_vec())
                .unwrap();
        assert_eq!(first.stream, source);
        assert_eq!(first.sequence, 123);
        assert_eq!(first.record.source_time, Some(source_time));
        assert_eq!(first.record.payload, native_payload);
        storage
            .write_stream(&[harmos::storage::StreamWrite {
                stream: &source,
                sequence: 124,
                record: harmos::storage::RecordWrite {
                    schema: &key,
                    scope: None,
                    item: &live,
                    source: Some(&source),
                    source_time: Some(source_time),
                    payload: &live_row_payload,
                },
            }])
            .await
            .unwrap();
        let storage_follow_stream_output::Value::Item(next) = following_rows
            .message()
            .await
            .unwrap()
            .unwrap()
            .value
            .unwrap()
        else {
            panic!("expected live row");
        };
        let next = <harmos::storage::StoredRow as harmos::Protobuf>::decode(&next.encode_to_vec())
            .unwrap();
        assert_eq!(next.sequence, 124);
        assert_eq!(next.record.payload, live_row_payload);
        // Native Production And Generated Readers Share The Original File Owner.
        use harmos::storage::files::FileBatch as FileBytes;
        use harmos::storage::follow::File as FileSelection;
        use harmos::storage::{FilePhase, FileProgress, StagedFile};
        use harmos_grpc_fixture::storage::v1::storage_follow_file_output;
        let mut producer = storage
            .produce_file(FileMetadata {
                id: Identifier::new("growing").unwrap(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            })
            .await
            .unwrap();
        producer.write_all(b"old").await.unwrap();
        use harmos::storage::operations::FileMutation;
        use harmos::storage::{FileProducer, MutationId, StorageInfo};
        let info = client
            .query_storage(storage_messages::query_storage::Row1::default())
            .await
            .unwrap()
            .into_inner();
        let info = <StorageInfo as harmos::Protobuf>::decode(&info.encode_to_vec()).unwrap();
        assert_eq!(info.accepted, storage.cursor_accepted());
        let request = FileMutation {
            mutation: MutationId {
                window: info.mutations.as_ref().unwrap().id.clone(),
                key: Identifier::new("resume-recording").unwrap(),
            },
            session: producer.session().unwrap().clone(),
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::resume_file::Row1::decode(encoded.as_slice()).unwrap();
        let resumed = client.resume_file(wire.clone()).await.unwrap().into_inner();
        let resumed = <FileProducer as harmos::Protobuf>::decode(&resumed.encode_to_vec()).unwrap();
        assert_eq!(resumed.session.generation, 2);
        assert_eq!(resumed.length, 3);
        let repeated = client.resume_file(wire).await.unwrap().into_inner();
        assert_eq!(
            <FileProducer as harmos::Protobuf>::decode(&repeated.encode_to_vec()).unwrap(),
            resumed
        );
        assert!(producer.write_all(b"stale").await.is_err());
        producer = storage.resume_file(&resumed.session).await.unwrap();
        let token = producer.session().unwrap();
        let mut selection = FileSelection {
            file: StagedFile {
                history: token.history.clone(),
                id: token.id.clone(),
            },
            offset: 0,
        };
        // Staging Bytes Remain Owned By The Production Follower.
        let mut reading = storage.follow_file(&selection.file, 0).await.unwrap();
        let mut prefix = Vec::new();
        while prefix.len() < 3 {
            let batch = reading
                .next_batch(NonZeroUsize::new(2).unwrap())
                .await
                .unwrap()
                .unwrap();
            assert!(batch.bytes.len() <= 2);
            prefix.extend_from_slice(&batch.bytes);
        }
        assert_eq!(prefix, b"old");
        assert_eq!(reading.progress().length, 3);
        assert_eq!(reading.progress().phase, FilePhase::Producing);
        selection.offset = 3;
        let encoded = harmos::Protobuf::encode_to_vec(&selection);
        let query = storage_messages::follow_file::Row1::decode(encoded.as_slice()).unwrap();
        let mut following_file = client.follow_file(query).await.unwrap().into_inner();
        producer.write_all(b"new").await.unwrap();
        let mut suffix = Vec::new();
        while suffix.len() < 3 {
            let storage_follow_file_output::Value::Item(chunk) = following_file
                .message()
                .await
                .unwrap()
                .unwrap()
                .value
                .unwrap()
            else {
                panic!("expected file bytes");
            };
            let chunk = <FileBytes as harmos::Protobuf>::decode(&chunk.encode_to_vec()).unwrap();
            assert_eq!(chunk.offset, 3 + suffix.len() as u64);
            suffix.extend_from_slice(&chunk.bytes);
        }
        assert_eq!(suffix, b"new");
        producer.finish().await.unwrap();
        let storage_follow_file_output::Value::Response(progress) = following_file
            .message()
            .await
            .unwrap()
            .unwrap()
            .value
            .unwrap()
        else {
            panic!("expected file completion");
        };
        let progress =
            <FileProgress as harmos::Protobuf>::decode(&progress.encode_to_vec()).unwrap();
        assert_eq!(progress.length, 6);
        assert_eq!(progress.phase, FilePhase::Finished);
        assert!(following_file.message().await.unwrap().is_none());
        use harmos::storage::operations::{AppendFileRequest, CreateFileRequest};
        let mutation = |key: &str| MutationId {
            window: info.mutations.as_ref().unwrap().id.clone(),
            key: Identifier::new(key).unwrap(),
        };
        for (name, finish) in [("remote-finished", true), ("remote-abandoned", false)] {
            let create = CreateFileRequest {
                mutation: mutation(name),
                metadata: FileMetadata {
                    id: Identifier::new(name).unwrap(),
                    scope: None,
                    media_type: "application/octet-stream".into(),
                    expected_length: Some(7),
                    expected_sha256: None,
                },
            };
            let encoded = harmos::Protobuf::encode_to_vec(&create);
            let wire = storage_messages::create_file::Row1::decode(encoded.as_slice()).unwrap();
            let created = client.create_file(wire).await.unwrap().into_inner();
            let created =
                <FileProducer as harmos::Protobuf>::decode(&created.encode_to_vec()).unwrap();
            let append = AppendFileRequest {
                mutation: mutation(&format!("{name}-append")),
                session: created.session.clone(),
                offset: 0,
                bytes: b"payload".to_vec(),
            };
            let encoded = harmos::Protobuf::encode_to_vec(&append);
            let wire = storage_messages::append_file::Row1::decode(encoded.as_slice()).unwrap();
            for _ in 0..2 {
                let appended = client.append_file(wire.clone()).await.unwrap().into_inner();
                let appended =
                    <FileProducer as harmos::Protobuf>::decode(&appended.encode_to_vec()).unwrap();
                assert_eq!(appended.length, 7);
                assert_eq!(appended.session, created.session);
            }
            let end = FileMutation {
                mutation: mutation(&format!("{name}-end")),
                session: created.session.clone(),
            };
            let encoded = harmos::Protobuf::encode_to_vec(&end);
            if finish {
                let wire = storage_messages::finish_file::Row1::decode(encoded.as_slice()).unwrap();
                let result = client.finish_file(wire.clone()).await.unwrap().into_inner();
                let reference =
                    <FileReference as harmos::Protobuf>::decode(&result.encode_to_vec()).unwrap();
                assert_eq!(reference.metadata.id.as_str(), name);
                assert_eq!(reference.length, 7);
                let replay = client.finish_file(wire).await.unwrap().into_inner();
                assert_eq!(
                    <FileReference as harmos::Protobuf>::decode(&replay.encode_to_vec()).unwrap(),
                    reference
                );
            } else {
                let wire =
                    storage_messages::abandon_file::Row1::decode(encoded.as_slice()).unwrap();
                let result = client.abandon_file(wire).await.unwrap().into_inner();
                let progress =
                    <FileProgress as harmos::Protobuf>::decode(&result.encode_to_vec()).unwrap();
                assert_eq!(progress.phase, FilePhase::Abandoned);
                assert_eq!(progress.length, 7);
            }
            let staged = StagedFile {
                history: created.session.history,
                id: created.session.id,
            };
            let mut native = storage.follow_file(&staged, 0).await.unwrap();
            let mut bytes = Vec::new();
            while let Some(batch) = native
                .next_batch(NonZeroUsize::new(3).unwrap())
                .await
                .unwrap()
            {
                bytes.extend_from_slice(&batch.bytes);
            }
            assert_eq!(bytes, b"payload");
            let deletion = harmos::storage::operations::DeleteFileRequest {
                mutation: MutationId {
                    window: info.mutations.as_ref().unwrap().id.clone(),
                    key: Identifier::new(format!("delete-{name}")).unwrap(),
                },
                file: staged.clone(),
            };
            let encoded = harmos::Protobuf::encode_to_vec(&deletion);
            let wire = storage_messages::delete_file::Row1::decode(encoded.as_slice()).unwrap();
            let result = client.delete_file(wire.clone()).await.unwrap().into_inner();
            assert_eq!(
                <StagedFile as harmos::Protobuf>::decode(&result.encode_to_vec()).unwrap(),
                staged
            );
            assert!(storage.follow_file(&staged, 0).await.is_err());
            let replay = client.delete_file(wire).await.unwrap().into_inner();
            assert_eq!(
                <StagedFile as harmos::Protobuf>::decode(&replay.encode_to_vec()).unwrap(),
                staged
            );
        }
        use arrow_schema::{DataType, Field, TimeUnit};
        use harmos::grpc::InputEnvelope;
        use harmos::storage::writes::{DataBatch, WriteRequest};
        let metadata = StructArray::new(
            vec![
                Field::new("item", DataType::Utf8, false),
                Field::new("source", DataType::Utf8, false),
                Field::new(
                    "source_time",
                    DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                    false,
                ),
                Field::new("sequence", DataType::UInt64, false),
            ]
            .into(),
            vec![
                Arc::new(StringArray::from(vec!["remote-record"])),
                Arc::new(StringArray::from(vec![source.as_str()])),
                Arc::new(TimestampNanosecondArray::from(vec![-100]).with_timezone("UTC")),
                Arc::new(UInt64Array::from(vec![456])),
            ],
            None,
        );
        let payload = StructArray::new(
            vec![Field::new("value", DataType::Utf8, false)].into(),
            vec![Arc::new(StringArray::from(vec!["remote bytes"]))],
            None,
        );
        let batch = write_batch(key.clone(), source.as_str(), metadata, payload);
        let frames = [
            InputEnvelope::<WriteRequest, DataBatch>::Request(WriteRequest { scope: None }),
            InputEnvelope::Item(batch.clone()),
        ];
        let frames = frames.into_iter().map(|frame| {
            harmos_grpc_fixture::storage::v1::StorageWriteRecordsInput::decode(
                harmos::Protobuf::encode_to_vec(&frame).as_slice(),
            )
            .unwrap()
        });
        let response = client
            .write_records(futures_util::stream::iter(frames))
            .await
            .unwrap()
            .into_inner();
        let accepted = write_completion(response).await;
        assert_eq!(storage.cursor_accepted(), accepted);
        assert_eq!(storage.cursor_saved(), accepted);
        let frames = [
            InputEnvelope::<WriteRequest, DataBatch>::Request(WriteRequest { scope: None }),
            InputEnvelope::Item(batch),
        ];
        let frames = frames.into_iter().map(|frame| {
            harmos_grpc_fixture::storage::v1::StorageWriteStreamsInput::decode(
                harmos::Protobuf::encode_to_vec(&frame).as_slice(),
            )
            .unwrap()
        });
        let response = client
            .write_streams(futures_util::stream::iter(frames))
            .await
            .unwrap()
            .into_inner();
        let stream_position = write_completion(response).await;
        assert_eq!(stream_position.sequence, accepted.sequence + 1);
        assert_eq!(storage.cursor_saved(), stream_position);
        use harmos::storage::SchemaInput;
        use harmos::storage::operations::WriteSchemaRequest;
        let logical = SchemaInput::new(
            harmos::storage::SchemaKey {
                payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
                id: Identifier::new("remote-schema").unwrap(),
                version: NonZeroU32::MIN,
            },
            &harmos::schema::graph::Schema {
                root: 0,
                definitions: vec![
                    vec![harmos::schema::graph::Field {
                        name: "nested".into(),
                        tag: 1,
                        cardinality: harmos::schema::Cardinality::Repeated,
                        kind: harmos::schema::graph::Kind::Message { definition: 1 },
                    }],
                    vec![harmos::schema::graph::Field {
                        name: "value".into(),
                        tag: 7,
                        cardinality: harmos::schema::Cardinality::Optional,
                        kind: harmos::schema::graph::Kind::String,
                    }],
                ],
            },
        )
        .unwrap();
        let request = WriteSchemaRequest {
            mutation: mutation("write-schema"),
            schema: logical.clone(),
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::write_schema::Row1::decode(encoded.as_slice()).unwrap();
        let response = client
            .write_schema(wire.clone())
            .await
            .unwrap()
            .into_inner();
        let position =
            <StorageCursor as harmos::Protobuf>::decode(&response.encode_to_vec()).unwrap();
        let replay = client.write_schema(wire).await.unwrap().into_inner();
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&replay.encode_to_vec()).unwrap(),
            position
        );
        assert_eq!(position.sequence, stream_position.sequence + 1);
        assert!(storage.cursor_saved().sequence < position.sequence);
        storage.flush().await.unwrap();
        let schemas = query_batches(
            &mut client,
            Query {
                at: position.clone(),
                source: Source::Schemas(harmos::storage::query::SchemaSelection {
                    schema: Some(logical.key.clone()),
                }),
                transforms: Vec::new(),
                slice: RowSlice {
                    offset: 0,
                    limit: Some(1),
                },
            },
        )
        .await;
        assert_eq!(schemas.len(), 1);
        let schema = schemas[0].as_record_batch();
        assert_eq!(schema.num_rows(), 1);
        assert_eq!(
            schema
                .column_by_name("schema_id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            logical.key.id.as_str()
        );
        let definition = schema
            .column_by_name("definition")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::BinaryArray>()
            .unwrap()
            .value(0);
        let decoded: harmos::schema::graph::Schema =
            <harmos::schema::graph::Schema as harmos::Message>::decode(definition).unwrap();
        assert_eq!(&decoded, logical.graph());
        use harmos::storage::ObjectCatalog;
        let value = StoredObjects::encode(
            &StoredDocument {
                id: "path/document".into(),
                text: "network".into(),
            },
            NonZeroUsize::new(4096).unwrap(),
        )
        .unwrap();
        let metadata = StructArray::new(
            vec![Field::new("item", DataType::Utf8, false)].into(),
            vec![Arc::new(StringArray::from(vec!["path/document"]))],
            None,
        );
        let payload = StructArray::new(
            vec![
                Field::new("id", DataType::Utf8, false),
                Field::new("text", DataType::Utf8, false),
            ]
            .into(),
            vec![
                Arc::new(StringArray::from(vec!["path/document"])),
                Arc::new(StringArray::from(vec!["network"])),
            ],
            None,
        );
        let batch = write_batch(
            value.schema.clone(),
            value.schema.id.as_str(),
            metadata,
            payload,
        );
        let frames = [
            InputEnvelope::<WriteRequest, DataBatch>::Request(WriteRequest { scope: None }),
            InputEnvelope::Item(batch),
        ];
        let frames = frames.into_iter().map(|frame| {
            harmos_grpc_fixture::storage::v1::StorageWriteObjectsInput::decode(
                harmos::Protobuf::encode_to_vec(&frame).as_slice(),
            )
            .unwrap()
        });
        let response = client
            .write_objects(futures_util::stream::iter(frames))
            .await
            .unwrap()
            .into_inner();
        let object_position = write_completion(response).await;
        assert!(object_position.sequence > position.sequence);
        assert_eq!(storage.cursor_saved(), object_position);
        let found = storage
            .read_object::<StoredDocument>(
                &"path/document".into(),
                harmos::storage::ObjectVersion::Latest,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.value.text, "network");
        let exact = storage
            .read_object::<StoredDocument>(
                &"path/document".into(),
                harmos::storage::ObjectVersion::Exact(found.version.clone()),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(exact.value.text, "network");
        let object_source = Source::Objects(harmos::storage::query::ObjectSelection {
            schema: Some(value.schema.clone()),
            key: Some(value.item.clone()),
            versions: harmos::storage::query::ObjectVersions::Latest,
        });
        let objects = query_batches(
            &mut client,
            Query {
                at: object_position.clone(),
                source: object_source.clone(),
                transforms: Vec::new(),
                slice: RowSlice {
                    offset: 0,
                    limit: Some(1),
                },
            },
        )
        .await;
        assert_eq!(objects.len(), 1);
        let object = objects[0].as_record_batch();
        assert_eq!(object.num_rows(), 1);
        let payload = object
            .column_by_name("payload")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(
            payload
                .column_by_name("id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "path/document"
        );
        assert_eq!(
            payload
                .column_by_name("text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "network"
        );
        use harmos::storage::query::transform::{
            FieldPath, Transform,
            project::{Project, Projection},
        };
        let projected = query_batches(
            &mut client,
            Query {
                at: object_position.clone(),
                source: object_source,
                transforms: vec![Transform::Project(Project {
                    fields: vec![Projection {
                        field: FieldPath {
                            fields: vec!["payload".into(), "text".into()],
                        },
                        name: "text".into(),
                    }],
                })],
                slice: RowSlice::default(),
            },
        )
        .await;
        assert_eq!(projected.len(), 1);
        let projected = projected[0].as_record_batch();
        assert_eq!(projected.num_columns(), 1);
        assert_eq!(
            projected
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "network"
        );

        use harmos_grpc_fixture::storage::v1::storage_follow_objects_output;
        let selection = harmos::storage::follow::Objects {
            after: Some(object_position),
            selection: harmos::storage::query::ObjectSelection {
                schema: None,
                key: None,
                versions: harmos::storage::query::ObjectVersions::Revisions,
            },
        };
        let wire = storage_messages::follow_objects::Row1::decode(
            harmos::Protobuf::encode_to_vec(&selection).as_slice(),
        )
        .unwrap();
        let mut following = client.follow_objects(wire).await.unwrap().into_inner();
        let live = storage
            .write_object(&StoredDocument {
                id: "path/document".into(),
                text: "live".into(),
            })
            .await
            .unwrap();
        let storage_follow_objects_output::Value::Item(item) =
            following.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected live object");
        };
        let native =
            <harmos::storage::ObjectEntry as harmos::Protobuf>::decode(&item.encode_to_vec())
                .unwrap();
        assert_eq!(native.version, live.version);
        drop(following);

        drop((
            following_file,
            reading,
            following_rows,
            records,
            rows,
            client,
        ));
        stop.send(()).unwrap();
        server.await.unwrap().unwrap();
        storage.close().await.unwrap();
        runtime.stop().await.unwrap();
    })
    .await
    .unwrap();
}
