/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Call Compound Mutation Through Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use crate::support::clock::Controlled;
use crate::support::{self, Bank, Ledger};
use harmos_domain::model::storage::catalog::object_schema;
use harmos_domain::model::storage::object_key;
use harmos_domain::model::storage::{
    CompoundOutcome, MutationId, MutationItem, ObjectCatalog, ObjectDefinition, ObjectRetention, ObjectValue, ObjectVersionToken,
    StoredObject,
};
use harmos_domain::{Identified, Message};
use harmos_operations::storage::{
    DeleteObject, DeleteObjectRequest, DeleteScope, DeleteScopeRequest, GetObject, GetObjectRequest, Mutate, MutateRequest,
};
use harmos_runtime::Identifier;
use harmos_runtime::storage::StorageManager;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, PartialEq)]
struct Counter {
    id: u64,
    value: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Identified for Counter {
    const ID: &'static str = "test.compound-counter";
}
impl StoredObject for Counter {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::All;
    fn key(&self) -> &u64 {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn compound_mutation_through_operations() {
    // Start with the Object Catalog and Register Native Operations.
    let clock = Controlled::new();
    let catalog = ObjectCatalog::new().entry(ObjectDefinition::of::<Counter>());
    let mut settings = crate::storage::config();
    settings.response.batch_bytes = NonZeroUsize::new(1536).unwrap();
    let storage = StorageManager::in_memory(catalog, settings).unwrap();
    let runtime = support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().clock(clock.source()).storage(&storage).register(support::catalog());
    })
    .await
    .unwrap();
    storage.start(&runtime).await.unwrap();
    support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    let value = Counter { id: 1, value: 10 };
    let key = object_key(&value.id, NonZeroUsize::MAX).unwrap();
    let object = ObjectValue {
        scope: None,
        schema: object_schema::<Counter>().unwrap(),
        item: key.clone(),
        payload: value.encode(),
    };
    let window = storage.mutation_window().unwrap().id;
    let mutation = MutationId {
        window: window.clone(),
        key: Identifier::new("create").unwrap(),
    };
    let record = crate::storage::record(&object.schema, &Identifier::new("created").unwrap(), &[7]);
    let request = MutateRequest {
        mutation: Some(mutation),
        items: vec![MutationItem::Create(object.clone()), MutationItem::Append(record)],
    };

    // Refuse An Oversized Reply Without Admitting Its Records.
    let before = storage.status().accepted;
    let oversized = MutateRequest {
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("oversized-reply").unwrap(),
        }),
        items: vec![MutationItem::Append(crate::storage::record(&object.schema, &Identifier::new("oversized").unwrap(), &[9])); 16],
    };
    assert!(runtime.operations.call_native::<Mutate>(oversized).await.is_err());
    assert_eq!(storage.status().accepted, before);

    // Mixed Writes Use the Registered Operation and Replay Its Durable Receipt.
    let reply = runtime.operations.call_native::<Mutate>(request.clone()).await.unwrap();
    let CompoundOutcome::Applied { positions } = &reply.outcome else { panic!("creation refused") };
    let version = ObjectVersionToken::new(positions[0].clone());
    assert!(reply.receipt.as_ref().unwrap().completed);
    assert_eq!(runtime.operations.call_native::<Mutate>(request).await.unwrap(), reply);
    let selected = runtime
        .operations
        .call_native::<GetObject>(GetObjectRequest {
            schema: Some(object.schema.clone()),
            item: key.to_string(),
            at: None,
        })
        .await
        .unwrap();
    assert_eq!(selected.value.unwrap().payload, value.encode());

    // Delete the Object and Append Its Event in the Same Commit.
    let record = crate::storage::record(&object.schema, &Identifier::new("deleted").unwrap(), &[8]);
    let request = MutateRequest {
        mutation: Some(MutationId {
            window,
            key: Identifier::new("delete").unwrap(),
        }),
        items: vec![
            MutationItem::Delete {
                schema: object.schema.id.clone(),
                item: key.clone(),
                expected: Some(version),
            },
            MutationItem::Append(record),
        ],
    };
    let deleted = runtime.operations.call_native::<Mutate>(request).await.unwrap();
    assert!(matches!(deleted.outcome, CompoundOutcome::Applied { .. }));
    let selected = runtime
        .operations
        .call_native::<GetObject>(GetObjectRequest {
            schema: Some(object.schema.clone()),
            item: key.to_string(),
            at: None,
        })
        .await
        .unwrap();
    assert!(!selected.exists);

    // Dedicated Delete Operations Use Their Registered Native Declarations.
    let receipt = runtime
        .operations
        .call_native::<DeleteObject>(DeleteObjectRequest {
            mutation: Some(MutationId {
                window: storage.mutation_window().unwrap().id,
                key: Identifier::new("dedicated-delete").unwrap(),
            }),
            schema: object.schema.id,
            item: key,
            expected: None,
        })
        .await
        .unwrap();
    assert!(matches!(receipt.outcome, CompoundOutcome::Applied { .. }));
    let deleted = runtime
        .operations
        .call_native::<DeleteScope>(DeleteScopeRequest {
            mutation: Some(MutationId {
                window: storage.mutation_window().unwrap().id,
                key: Identifier::new("dedicated-scope-delete").unwrap(),
            }),
            scope: Identifier::new("empty-scope").unwrap(),
        })
        .await
        .unwrap();
    assert!(matches!(deleted.outcome, CompoundOutcome::Applied { .. }));
    storage.close().await.unwrap();
    support::app::shutdown(&runtime).await.unwrap();
}
