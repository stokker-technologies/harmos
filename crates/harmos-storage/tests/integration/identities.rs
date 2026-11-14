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
};

// Third Party Imports

// Local Imports
use harmos_domain::Message;
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
    let backend = memory::Backend::new(history.clone());
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

