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
use prost::Message as _;

// Local Imports
use harmos::storage::operations::{
    DataSelection, Followed, Operations, Save, SaveRequest, Selection,
};
use harmos::storage::{
    FileMetadata, FileReference, StorageBackends, StorageCursor, StorageManager, StorageSettings,
};
use harmos::{Identifier, Runtime};
use harmos_grpc_fixture::storage::v1::{
    storage_client::StorageClient, storage_follow_output, storage_follow_stream_output,
    storage_messages, storage_query_files_output, storage_query_schemas_output,
    storage_query_stream_output, storage_read_records_output,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
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
            StorageManager::new(
                StorageBackends {
                    primary: data.clone(),
                    recovery: Arc::new(backend::Recovery::new(backend::history())),
                    replicas: Vec::new(),
                },
                StorageSettings {
                    flush_interval: Duration::from_secs(60),
                    ..StorageSettings::default()
                },
            )
            .unwrap(),
        );
        storage.start(&runtime.services).await.unwrap();
        Operations::register(
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
            payload: b"native bytes",
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
        assert!(storage.cursor_saved().sequence < through.sequence);
        let capacity = harmos::operations::StreamSettings {
            items: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(4096).unwrap(),
        };
        let service = harmos_grpc_fixture::StorageServer::new(
            harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
            harmos::grpc::server::Settings {
                calls: NonZeroUsize::new(4).unwrap(),
                decode: harmos::grpc::DecodeSettings {
                    message_bytes: NonZeroUsize::new(4096).unwrap(),
                    retained_bytes: NonZeroUsize::new(16384).unwrap(),
                },
                encode: harmos::grpc::EncodeSettings {
                    message_bytes: NonZeroUsize::new(4096).unwrap(),
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
        let save = SaveRequest {
            mutation: harmos::storage::MutationId {
                window: storage.mutation_window().unwrap().id,
                key: Identifier::new("initial-save").unwrap(),
            },
            through: through.clone(),
        };
        let encoded = harmos::Protobuf::encode_to_vec(&save);
        let request = storage_messages::save::Row1::decode(encoded.as_slice()).unwrap();
        let saved = client
            .save(request)
            .await
            .unwrap()
            .into_inner()
            .encode_to_vec();
        let saved = <StorageCursor as harmos::Protobuf>::decode(&saved).unwrap();
        assert_eq!(saved, storage.cursor_saved());
        let local = runtime.operations.call_as::<Save>(save).await.unwrap();
        assert_eq!(*local, saved);
        let encoded = harmos::Protobuf::encode_to_vec(&Selection::default());
        let query = storage_messages::query_files::Row1::decode(encoded.as_slice()).unwrap();
        let mut stream = client.query_files(query).await.unwrap().into_inner();
        let item = stream.message().await.unwrap().unwrap().value.unwrap();
        let storage_query_files_output::Value::Item(item) = item else {
            panic!("expected file item")
        };
        let actual = <FileReference as harmos::Protobuf>::decode(&item.encode_to_vec()).unwrap();
        assert_eq!(actual, file);
        let completed = stream.message().await.unwrap().unwrap().value.unwrap();
        let storage_query_files_output::Value::Response(completed) = completed else {
            panic!("expected final cursor")
        };
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&completed.encode_to_vec()).unwrap(),
            saved
        );
        assert!(stream.message().await.unwrap().is_none());
        let selection = DataSelection {
            selection: Selection::default(),
            schema: key.clone(),
            stream: None,
            source_start: Some(std::time::UNIX_EPOCH - Duration::from_secs(1)),
            source_end: Some(std::time::UNIX_EPOCH),
            record_filter: None,
        };
        let encoded = harmos::Protobuf::encode_to_vec(&selection);
        let query = storage_messages::read_records::Row1::decode(encoded.as_slice()).unwrap();
        let mut records = client.read_records(query).await.unwrap().into_inner();
        let storage_read_records_output::Value::Item(record) =
            records.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected record");
        };
        let record =
            <harmos::storage::StoredRecord as harmos::Protobuf>::decode(&record.encode_to_vec())
                .unwrap();
        assert_eq!(record.schema, key);
        assert_eq!(record.item, sample);
        assert_eq!(record.source, Some(source.clone()));
        assert_eq!(record.source_time, Some(source_time));
        assert_eq!(record.payload, b"native bytes");
        let storage_read_records_output::Value::Response(completed) =
            records.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected record boundary");
        };
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&completed.encode_to_vec()).unwrap(),
            saved
        );
        assert!(records.message().await.unwrap().is_none());
        let query = storage_messages::query_stream::Row1::decode(encoded.as_slice()).unwrap();
        let mut rows = client.query_stream(query).await.unwrap().into_inner();
        let storage_query_stream_output::Value::Item(row) =
            rows.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected stream row");
        };
        let row =
            <harmos::storage::StoredRow as harmos::Protobuf>::decode(&row.encode_to_vec()).unwrap();
        assert_eq!(row.stream, source);
        assert_eq!(row.sequence, 123);
        assert_eq!(row.record.source_time, Some(source_time));
        assert_eq!(row.record.payload, b"native bytes");
        let storage_query_stream_output::Value::Response(completed) =
            rows.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected stream boundary");
        };
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&completed.encode_to_vec()).unwrap(),
            saved
        );
        assert!(rows.message().await.unwrap().is_none());
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
        assert_eq!(followed(&first.encode_to_vec()), record);
        let live = Identifier::new("live").unwrap();
        let position = storage
            .write_records(&[harmos::storage::RecordWrite {
                schema: &key,
                scope: None,
                item: &live,
                source: Some(&source),
                source_time: Some(source_time),
                payload: b"live bytes",
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
        assert_eq!(first, row);
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
                    payload: b"live row",
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
        assert_eq!(next.record.payload, b"live row");
        // Native Production And Generated Readers Share The Original File Owner.
        use harmos::storage::operations::{FileBytes, FileSelection};
        use harmos::storage::{FilePhase, FileProgress, StagedFile};
        use harmos_grpc_fixture::storage::v1::{
            storage_follow_file_output, storage_read_files_output,
        };
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
            chunk_bytes: 2,
        };
        let encoded = harmos::Protobuf::encode_to_vec(&selection);
        let query = storage_messages::read_files::Row1::decode(encoded.as_slice()).unwrap();
        let mut reading = client.read_files(query).await.unwrap().into_inner();
        let mut prefix = Vec::new();
        loop {
            match reading.message().await.unwrap().unwrap().value.unwrap() {
                storage_read_files_output::Value::Item(chunk) => {
                    let chunk =
                        <FileBytes as harmos::Protobuf>::decode(&chunk.encode_to_vec()).unwrap();
                    assert_eq!(chunk.offset, prefix.len() as u64);
                    assert!(chunk.bytes.len() <= 2);
                    prefix.extend_from_slice(&chunk.bytes);
                }
                storage_read_files_output::Value::Response(progress) => {
                    let progress =
                        <FileProgress as harmos::Protobuf>::decode(&progress.encode_to_vec())
                            .unwrap();
                    assert_eq!(progress.length, 3);
                    assert_eq!(progress.phase, FilePhase::Producing);
                    break;
                }
            }
        }
        assert_eq!(prefix, b"old");
        assert!(reading.message().await.unwrap().is_none());
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
            while let Some(chunk) = native
                .next_chunk(NonZeroUsize::new(3).unwrap())
                .await
                .unwrap()
            {
                bytes.extend_from_slice(chunk.as_bytes());
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
        use harmos::storage::operations::{WriteRecordsRequest, WriteStreamRequest};
        use harmos::storage::{RecordInput, StreamInput};
        let input = RecordInput {
            schema: key.clone(),
            scope: None,
            item: Identifier::new("remote-record").unwrap(),
            source: Some(source.clone()),
            source_time: Some(source_time),
            payload: b"remote bytes".to_vec(),
        };
        let request = WriteRecordsRequest {
            mutation: mutation("remote-record-write"),
            records: vec![input.clone()],
            source: source.clone(),
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::write_records::Row1::decode(encoded.as_slice()).unwrap();
        let response = client
            .write_records(wire.clone())
            .await
            .unwrap()
            .into_inner();
        let accepted =
            <StorageCursor as harmos::Protobuf>::decode(&response.encode_to_vec()).unwrap();
        let response = client.write_records(wire).await.unwrap().into_inner();
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&response.encode_to_vec()).unwrap(),
            accepted
        );
        assert_eq!(storage.cursor_accepted(), accepted);
        let request = WriteStreamRequest {
            mutation: mutation("remote-stream-write"),
            rows: vec![StreamInput {
                record: input,
                stream: source.clone(),
                sequence: 456,
            }],
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::write_stream::Row1::decode(encoded.as_slice()).unwrap();
        let response = client.write_stream(wire).await.unwrap().into_inner();
        let stream_position =
            <StorageCursor as harmos::Protobuf>::decode(&response.encode_to_vec()).unwrap();
        assert_eq!(stream_position.sequence, accepted.sequence + 1);
        use harmos::storage::operations::{QuerySchemasRequest, WriteSchemaRequest};
        use harmos::storage::{SchemaInfo, SchemaInput};
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
        let query = QuerySchemasRequest {
            schema: Some(logical.key.clone()),
            selection: Selection {
                page_rows: 1,
                ..Default::default()
            },
        };
        let encoded = harmos::Protobuf::encode_to_vec(&query);
        let wire = storage_messages::query_schemas::Row1::decode(encoded.as_slice()).unwrap();
        let mut schemas = client.query_schemas(wire).await.unwrap().into_inner();
        let storage_query_schemas_output::Value::Item(item) =
            schemas.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected schema");
        };
        let native = <SchemaInfo as harmos::Protobuf>::decode(&item.encode_to_vec()).unwrap();
        assert_eq!(
            native,
            SchemaInfo {
                position: position.clone(),
                schema: logical
            }
        );
        let storage_query_schemas_output::Value::Response(completed) =
            schemas.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected boundary");
        };
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&completed.encode_to_vec()).unwrap(),
            position
        );
        assert!(schemas.message().await.unwrap().is_none());
        use harmos::storage::ObjectCatalog;
        use harmos::storage::operations::{WriteObjectsRequest, WriteObjectsResponse};
        let value = StoredObjects::encode(
            &StoredDocument {
                id: "path/document".into(),
                text: "network".into(),
            },
            NonZeroUsize::new(4096).unwrap(),
        )
        .unwrap();
        let request = WriteObjectsRequest {
            mutation: mutation("write-objects"),
            objects: vec![value],
        };
        let encoded = harmos::Protobuf::encode_to_vec(&request);
        let wire = storage_messages::write_objects::Row1::decode(encoded.as_slice()).unwrap();
        let response = client
            .write_objects(wire.clone())
            .await
            .unwrap()
            .into_inner();
        let admitted =
            <WriteObjectsResponse as harmos::Protobuf>::decode(&response.encode_to_vec()).unwrap();
        assert_eq!(admitted.objects[0].position.sequence, position.sequence + 2);
        let repeated = client.write_objects(wire).await.unwrap().into_inner();
        assert_eq!(
            <WriteObjectsResponse as harmos::Protobuf>::decode(&repeated.encode_to_vec()).unwrap(),
            admitted
        );
        assert_eq!(
            storage
                .write_objects_once(&request.mutation, &request.objects)
                .await
                .unwrap(),
            admitted.objects
        );
        storage.flush().await.unwrap();
        let found = storage
            .read_object::<StoredDocument>(
                &"path/document".into(),
                harmos::storage::ObjectVersion::Exact(admitted.objects[0].version.clone()),
                NonZeroUsize::new(4096).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.value.text, "network");
        use harmos::storage::operations::{ObjectResult, ObjectSelection, ReadObjectsRequest};
        use harmos_grpc_fixture::storage::v1::{
            storage_follow_objects_output, storage_read_objects_output,
        };
        let mut selection = ObjectSelection::default();
        selection.selection.page_rows = 1;
        let wire = storage_messages::read_objects::Row1::decode(
            harmos::Protobuf::encode_to_vec(&ReadObjectsRequest {
                selection: selection.clone(),
                fields: None,
            })
            .as_slice(),
        )
        .unwrap();
        let mut objects = client.read_objects(wire).await.unwrap().into_inner();
        let storage_read_objects_output::Value::Item(item) =
            objects.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected object");
        };
        let ObjectResult::Value(native) =
            <ObjectResult as harmos::Protobuf>::decode(&item.encode_to_vec()).unwrap()
        else {
            panic!("expected full object");
        };
        assert_eq!(native.version, admitted.objects[0].version);
        assert_eq!(native.value, request.objects[0]);
        let storage_read_objects_output::Value::Response(boundary) =
            objects.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected boundary");
        };
        assert_eq!(
            <StorageCursor as harmos::Protobuf>::decode(&boundary.encode_to_vec()).unwrap(),
            storage.cursor_saved()
        );
        assert!(objects.message().await.unwrap().is_none());
        let row = harmos::storage::ObjectMetadata {
            schema: native.value.schema.clone(),
            item: native.value.item.clone(),
            position: native.position.clone(),
            version: native.version.clone(),
            values: vec![harmos::storage::ObjectScalar::Text("network".into())],
        };
        *data.control.browse_rows.lock().unwrap() = Some(vec![row.clone()]);
        let projected = ReadObjectsRequest {
            selection: ObjectSelection {
                schema: Some(native.value.schema.clone()),
                ..Default::default()
            },
            fields: Some(harmos::storage::ObjectFields {
                projection: vec![Identifier::new("text").unwrap()],
                ..Default::default()
            }),
        };
        let wire = storage_messages::read_objects::Row1::decode(
            harmos::Protobuf::encode_to_vec(&projected).as_slice(),
        )
        .unwrap();
        let mut projected = client.read_objects(wire).await.unwrap().into_inner();
        let storage_read_objects_output::Value::Item(item) =
            projected.message().await.unwrap().unwrap().value.unwrap()
        else {
            panic!("expected scalar metadata");
        };
        assert_eq!(
            <ObjectResult as harmos::Protobuf>::decode(&item.encode_to_vec()).unwrap(),
            ObjectResult::Fields(row)
        );
        assert!(matches!(
            projected.message().await.unwrap().unwrap().value.unwrap(),
            storage_read_objects_output::Value::Response(_)
        ));
        assert!(projected.message().await.unwrap().is_none());

        selection.versions = harmos::storage::ObjectVersions::Revisions;
        selection.selection.after = Some(native.position);
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
            stream,
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
