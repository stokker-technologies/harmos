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
use std::{
    num::NonZeroUsize,
    time::{Duration, UNIX_EPOCH},
};

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos_domain::Message;
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
struct Reading {
    value: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn stream_identities_keep_their_first_position_and_snapshot_selection() {
    use harmos_domain::schema::Described;
    use std::num::NonZeroU32;

    // Persist Two Scoped Identities and Repeated Rows of the First Identity.
    let history = StoreHistory {
        store: Identifier::new("store").unwrap(),
        epoch: Identifier::new("epoch").unwrap(),
    };
    let backend = SqliteBackend::in_memory(
        history.clone(),
        SqliteSettings {
            chunk_rows: nz(4),
            chunk_bytes: nz(65536),
        },
    )
    .unwrap();
    backend.create_session().await.unwrap();
    let key = SchemaKey {
        payload_encoding: PayloadEncoding::PostcardV1,
        id: Identifier::new("reading").unwrap(),
        version: NonZeroU32::MIN,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(Reading::DEFINITION, &Default::default()).unwrap();
    backend
        .append_schemas(
            StorageBatch::new(vec![StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, key.clone(), graph)]).into(),
        )
        .await
        .unwrap();
    let mut rows = Vec::new();
    for (position, scope) in [(2, "first"), (3, "first"), (4, "second")] {
        rows.push(StoredRow {
            record: StoredRecord {
                position: StorageCursor {
                    history: history.clone(),
                    sequence: position,
                },
                schema: key.clone(),
                scope: Some(Identifier::new(scope).unwrap()),
                item: Identifier::new("item").unwrap(),
                source: None,
                source_time: None,
                payload: Reading { value: position }.encode(),
            },
            stream: Identifier::new("readings").unwrap(),
            sequence: position,
        });
    }
    backend.append_samples(StorageBatch::new(rows)).await.unwrap();

    // A Retained Cut Excludes the Later Identity; Continuations Use First Positions.
    let early = backend.create_snapshot(&StorageCursor { history: history.clone(), sequence: 3 }).await.unwrap();
    let current = backend.create_snapshot(&StorageCursor { history, sequence: 4 }).await.unwrap();
    let selection = IdentitySelection {
        kind: IdentityKind::Streams,
        schema: Some(key.id.clone()),
    };
    let page = ScanPage { after: 0, rows: nz(1), bytes: nz(4096) };
    let first = backend.list_identities(&early, &selection, page).await.unwrap();
    assert_eq!(first.len(), 1);
    let StoredIdentity::Stream {
        schema,
        scope,
        item,
        stream,
        first: position,
    } = &first[0]
    else {
        panic!("Expected Stream Identity")
    };
    assert_eq!(schema, &key);
    assert_eq!(scope.as_ref().unwrap().as_str(), "first");
    assert_eq!(item.as_str(), "item");
    assert_eq!(stream.as_str(), "readings");
    assert_eq!(position.sequence, 2);
    let after = ScanPage { after: position.sequence, ..page };
    assert!(backend.list_identities(&early, &selection, after).await.unwrap().is_empty());
    let later = backend.list_identities(&current, &selection, after).await.unwrap();
    assert_eq!(later.len(), 1);
    assert_eq!(later[0].first().sequence, 4);
    assert!(
        backend
            .list_identities(
                &current,
                &IdentitySelection {
                    schema: Some(Identifier::new("absent").unwrap()),
                    ..selection
                },
                page
            )
            .await
            .unwrap()
            .is_empty()
    );
    backend.delete_snapshot(&early).await.unwrap();
    backend.delete_snapshot(&current).await.unwrap();
    backend.delete_session().await.unwrap();
}

#[tokio::test]
async fn finished_files_and_mutation_receipts_reopen_without_resetting_incompatible_roots() {
    // Persist Ordered Chunks, Refuse Conflicting Replay And Verify Completion.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("files.sqlite");
    let history = StoreHistory {
        store: Identifier::new("store").unwrap(),
        epoch: Identifier::new("epoch").unwrap(),
    };
    let settings = SqliteSettings {
        chunk_rows: nz(4),
        chunk_bytes: nz(65536),
    };
    let backend = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    backend.create_session().await.unwrap();
    let content = b"abcdefgh";
    let sha256 = Sha256::digest(content).into();
    let metadata = FileMetadata {
        id: Identifier::new("file").unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(content.len() as u64),
        expected_sha256: Some(sha256),
    };
    let file = backend.create_file_staging(&metadata).await.unwrap();
    backend.append_file_bytes(&file, 0, StorageBatch::new(content[..3].to_vec())).await.unwrap();
    backend.append_file_bytes(&file, 3, StorageBatch::new(content[3..].to_vec())).await.unwrap();
    backend.append_file_bytes(&file, 3, StorageBatch::new(content[3..].to_vec())).await.unwrap();
    assert!(backend.append_file_bytes(&file, 3, StorageBatch::new(vec![0; content.len() - 3])).await.is_err());
    let cut = StorageCursor { history: history.clone(), sequence: 1 };
    let mut wrong = sha256;
    wrong[0] ^= 1;
    assert!(backend.publish_file_production(&file, &cut, content.len() as u64, wrong).await.is_err());
    let reference = backend.publish_file_production(&file, &cut, content.len() as u64, sha256).await.unwrap();
    backend.append_file_references(StorageBatch::new(vec![reference.clone()])).await.unwrap();
    backend.append_file_references(StorageBatch::new(vec![reference.clone()])).await.unwrap();
    assert_eq!(backend.publish_file_production(&file, &cut, content.len() as u64, sha256).await.unwrap(), reference);
    let saved = backend.create_snapshot(&cut).await.unwrap();
    assert_eq!(backend.get_file_bytes(&saved, &reference, 2, nz(4)).await.unwrap(), content[2..6]);
    assert!(backend.delete_file_staging(&file).await.is_err());

    // Save And Replay One Durable Mutation Receipt.
    let receipt = MutationReceipt {
        mutation: MutationId {
            window: Identifier::new("window").unwrap(),
            key: Identifier::new("publish").unwrap(),
        },
        fingerprint: Sha256::digest(content).into(),
        expires_at: UNIX_EPOCH + Duration::from_secs(1000),
        value: MutationValue::Cursor(cut.clone()),
    };
    backend.append_mutation_receipts(StorageBatch::new(vec![receipt.clone()])).await.unwrap();
    backend.append_mutation_receipts(StorageBatch::new(vec![receipt.clone()])).await.unwrap();
    let mut conflict = receipt.clone();
    conflict.fingerprint[0] ^= 1;
    assert!(backend.append_mutation_receipts(StorageBatch::new(vec![conflict])).await.is_err());
    backend.delete_snapshot(&saved).await.unwrap();
    backend.delete_session().await.unwrap();
    drop(backend);

    // Refuse A Different History Before Modifying The Database.
    let bytes = std::fs::read(&path).unwrap();
    let mut different = history.clone();
    different.epoch = Identifier::new("different").unwrap();
    let error = SqliteBackend::new(&path, different, settings).err().unwrap();
    assert_eq!(error.backend_failure(), Some(BackendFailure::Corrupt));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let backend = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    backend.create_session().await.unwrap();
    let snapshot = backend.create_snapshot(&cut).await.unwrap();
    assert_eq!(backend.get_file_bytes(&snapshot, &reference, 0, nz(content.len())).await.unwrap(), content);
    assert_eq!(backend.list_mutation_receipts(ReadLimits { rows: nz(4), bytes: nz(4096) }).await.unwrap(), vec![receipt]);

    // Foreign Tokens At The Same History And Cut Must Still Refuse.
    let other = SqliteBackend::new(root.path().join("other.sqlite"), history.clone(), settings).unwrap();
    other.create_session().await.unwrap();
    let staged = other.create_file_staging(&metadata).await.unwrap();
    other.append_file_bytes(&staged, 0, StorageBatch::new(content.to_vec())).await.unwrap();
    let published = other.publish_file_production(&staged, &cut, content.len() as u64, sha256).await.unwrap();
    other.append_file_references(StorageBatch::new(vec![published.clone()])).await.unwrap();
    let foreign = other.create_snapshot(&cut).await.unwrap();
    assert!(backend.get_file_bytes(&foreign, &reference, 0, nz(1)).await.is_err());
    assert!(other.get_file_bytes(&snapshot, &published, 0, nz(1)).await.is_err());
    other.delete_snapshot(&foreign).await.unwrap();
    other.delete_session().await.unwrap();
    backend.delete_snapshot(&snapshot).await.unwrap();
    backend.delete_session().await.unwrap();
    drop(other);
    drop(backend);

    // Preserve A Non-Harmos Database Byte For Byte Instead Of Resetting It.
    let incompatible = root.path().join("incompatible.sqlite");
    let connection = rusqlite::Connection::open(&incompatible).unwrap();
    connection.execute_batch("CREATE TABLE evidence(value BLOB); INSERT INTO evidence VALUES(x'010203');").unwrap();
    drop(connection);
    let bytes = std::fs::read(&incompatible).unwrap();
    let error = SqliteBackend::new(&incompatible, history, settings).err().unwrap();
    assert_eq!(error.backend_failure(), Some(BackendFailure::Corrupt));
    assert_eq!(std::fs::read(&incompatible).unwrap(), bytes);
}
