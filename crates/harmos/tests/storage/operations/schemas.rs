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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, config};
use harmos::Identifier;
use harmos::schema::Cardinality;
use harmos::schema::graph::{Field, Kind, Schema};
use harmos::storage::operations::{
    Operations, QuerySchemas, QuerySchemasRequest, Selection, WriteSchema, WriteSchemaRequest,
};
use harmos::storage::{MutationId, SchemaInfo, SchemaInput, SchemaKey};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Binding<D>(std::marker::PhantomData<D>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<D: harmos::operations::OperationDeclaration> harmos::operations::Operation for Binding<D> {
    type Declaration = D;
    type Resources = ();
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = harmos::operations::Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos::Result<Self> {
        Ok(Self(std::marker::PhantomData))
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut D::Request,
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<D::Response>> {
        let result: ::core::result::Result<D::Response, Self::Error> =
            async move { unreachable!("retired contract must be rejected before execution") }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn schema(name: &str) -> SchemaInput {
    SchemaInput::new(
        SchemaKey {
            payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
            id: Identifier::new(name).unwrap(),
            version: NonZeroU32::MIN,
        },
        &Schema {
            root: 0,
            definitions: vec![
                vec![Field {
                    name: Cow::Borrowed("rows"),
                    tag: 1,
                    cardinality: Cardinality::Repeated,
                    kind: Kind::Message { definition: 1 },
                }],
                vec![
                    Field {
                        name: Cow::Borrowed("value"),
                        tag: 1,
                        cardinality: Cardinality::Required,
                        kind: Kind::F64,
                    },
                    Field {
                        name: Cow::Borrowed("next"),
                        tag: 2,
                        cardinality: Cardinality::Optional,
                        kind: Kind::Message { definition: 1 },
                    },
                ],
            ],
        },
    )
    .unwrap()
}
fn request(f: &Fixture, name: &str) -> WriteSchemaRequest {
    WriteSchemaRequest {
        mutation: MutationId {
            window: f.storage.mutation_window().unwrap().id,
            key: Identifier::new(name).unwrap(),
        },
        schema: schema(name),
    }
}
async fn setup() -> Fixture {
    let f = Fixture::new(config()).await;
    Operations::register(
        &f.runtime.operations,
        "storage",
        Lanes::Open,
        Arc::new(f.storage.clone()),
    )
    .unwrap();
    f
}
async fn query(f: &Fixture, request: QuerySchemasRequest) -> Vec<SchemaInfo> {
    let (sender, mut receiver) =
        harmos::operations::stream::<SchemaInfo>(harmos::operations::StreamSettings {
            items: NonZeroUsize::MIN,
            bytes: NonZeroUsize::new(16 * 1024).unwrap(),
        })
        .unwrap();
    let call = f
        .runtime
        .operations
        .submit_endpoints::<QuerySchemas, _>(request, sender)
        .unwrap();
    let mut schemas = Vec::new();
    while let Some(schema) = receiver.recv().await {
        schemas.push((*schema).clone());
    }
    assert_eq!(*call.await.unwrap(), f.storage.cursor_saved());
    schemas
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("schema_writes_share", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = setup().await;
                    let declared = harmos::operations::WireDeclaration::of::<WriteSchema>()
                        .contract()
                        .unwrap();
                    assert_eq!(declared.version(), 2);
                    let retired = harmos::operations::Contract::new(
                        declared.operation().clone(),
                        1,
                        declared.codec(),
                        declared.schema().clone(),
                    )
                    .unwrap();
                    assert!(!declared.compatible(&retired));
                    assert!(
                        f.runtime
                            .operations
                            .register_wire_contract::<Binding<WriteSchema>>(
                                "schema-v1",
                                &retired,
                                Lanes::Open,
                                Arc::new(())
                            )
                            .is_err()
                    );
                    let declared = harmos::operations::WireDeclaration::of::<QuerySchemas>()
                        .contract()
                        .unwrap();
                    assert_eq!(declared.version(), 2);
                    let retired = harmos::operations::Contract::new(
                        declared.operation().clone(),
                        1,
                        declared.codec(),
                        declared.schema().clone(),
                    )
                    .unwrap();
                    assert!(!declared.compatible(&retired));
                    assert!(
                        f.runtime
                            .operations
                            .register_wire_contract::<Binding<QuerySchemas>>(
                                "schema-v1",
                                &retired,
                                Lanes::Open,
                                Arc::new(())
                            )
                            .is_err()
                    );
                    let request = request(&f, "schema");
                    let cursor = f
                        .storage
                        .write_schema_once(
                            &request.mutation,
                            &request.schema.key,
                            request.schema.graph(),
                        )
                        .await
                        .unwrap();
                    let repeated = f
                        .runtime
                        .operations
                        .call_as::<WriteSchema>(request.clone())
                        .await
                        .unwrap();
                    assert_eq!(*repeated, cursor);
                    assert_eq!(f.storage.cursor_accepted(), cursor);
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert!(query(&f, QuerySchemasRequest::default()).await.is_empty());
                    let mut conflict = request.clone();
                    conflict.schema.key.version = NonZeroU32::new(2).unwrap();
                    assert!(
                        f.runtime
                            .operations
                            .call_as::<WriteSchema>(conflict)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted(), cursor);
                    f.storage.flush().await.unwrap();
                    let values = query(&f, QuerySchemasRequest::default()).await;
                    assert_eq!(
                        values,
                        vec![SchemaInfo {
                            position: cursor,
                            schema: request.schema
                        }]
                    );
                    assert!(
                        f.data
                            .mutation_receipts
                            .lock()
                            .unwrap()
                            .contains_key(&request.mutation)
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("discovery_preserves_filters", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = setup().await;
                    for name in ["first", "second", "third"] {
                        f.runtime
                            .operations
                            .call_as::<WriteSchema>(request(&f, name))
                            .await
                            .unwrap();
                    }
                    f.storage.flush().await.unwrap();
                    let selection = Selection {
                        descending: true,
                        page_rows: 1,
                        page_bytes: 4096,
                        ..Default::default()
                    };
                    let values = query(
                        &f,
                        QuerySchemasRequest {
                            selection: selection.clone(),
                            schema: None,
                        },
                    )
                    .await;
                    assert_eq!(
                        values
                            .iter()
                            .map(|value| value.schema.key.id.as_str())
                            .collect::<Vec<_>>(),
                        ["third", "second", "first"]
                    );
                    let selected = query(
                        &f,
                        QuerySchemasRequest {
                            selection,
                            schema: Some(schema("second").key),
                        },
                    )
                    .await;
                    assert_eq!(selected.len(), 1);
                    assert_eq!(selected[0].schema.key.id.as_str(), "second");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("schema_retry_conflicts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = setup().await;
                    let request = request(&f, "nested");
                    f.runtime
                        .operations
                        .call_as::<WriteSchema>(request.clone())
                        .await
                        .unwrap();
                    let mut changed = request.schema.graph().clone();
                    changed.definitions[1][0].kind = Kind::String;
                    let modified = SchemaInput::new(request.schema.key.clone(), &changed).unwrap();
                    assert!(
                        f.runtime
                            .operations
                            .call_as::<WriteSchema>(WriteSchemaRequest {
                                mutation: request.mutation.clone(),
                                schema: modified
                            })
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted().sequence, 1);
                    let mut invalid = request.schema.graph().clone();
                    invalid.definitions[invalid.root][0].tag = 0;
                    assert!(
                        f.storage
                            .write_schema_once(&request.mutation, &request.schema.key, &invalid)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted().sequence, 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("schema_retry_survives", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = setup().await;
                    let request = request(&f, "accepted");
                    let cursor = f
                        .runtime
                        .operations
                        .call_as::<WriteSchema>(request.clone())
                        .await
                        .unwrap();
                    f.data
                        .control
                        .failures
                        .store(1, std::sync::atomic::Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    let replay = f
                        .runtime
                        .operations
                        .call_as::<WriteSchema>(request)
                        .await
                        .unwrap();
                    assert_eq!(*replay, *cursor);
                    f.storage.flush().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("query_hides_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let settings = config();
                    let runtime =
                        crate::Runtime::<crate::Bank>::builder(crate::Ledger::default(), ())
                            .register(crate::catalog())
                            .register(Lanes::catalog())
                            .runtime()
                            .await
                            .unwrap();
                    let data = crate::Data::new(crate::backend::history());
                    let data = Arc::new(data);
                    let recovery = Arc::new(crate::Recovery::new(crate::backend::history()));
                    let storage = harmos::storage::StorageManager::new(
                        harmos::storage::StorageBackends {
                            primary: data.clone(),
                            recovery: recovery.clone(),
                            replicas: Vec::new(),
                        },
                        settings.clone(),
                    )
                    .unwrap();
                    storage.start(&runtime.services).await.unwrap();
                    let f = Fixture {
                        runtime,
                        storage,
                        data,
                        recovery,
                        execution: settings.query_execution,
                    };
                    Operations::register(
                        &f.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    let request = request(&f, "graph-schema");
                    f.runtime
                        .operations
                        .call_as::<WriteSchema>(request.clone())
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let persisted = f
                        .data
                        .schemas
                        .lock()
                        .unwrap()
                        .values()
                        .next()
                        .unwrap()
                        .clone();
                    let metadata = serde_json::to_value(&persisted).unwrap();
                    assert_eq!(metadata["encoding"], "graph_v1");
                    let restored: harmos::storage::StorageSchema =
                        serde_json::from_value(metadata.clone()).unwrap();
                    assert_eq!(restored.schema, *request.schema.graph());
                    let mut retired = metadata;
                    retired["encoding"] = "inline".into();
                    let error = serde_json::from_value::<harmos::storage::StorageSchema>(retired)
                        .unwrap_err();
                    assert!(error.to_string().contains("select a fresh storage root"));
                    let rows = query(&f, QuerySchemasRequest::default()).await;
                    assert_eq!(rows[0].schema, request.schema);
                    let (sender, mut receiver) = harmos::operations::stream::<SchemaInfo>(
                        harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::MIN,
                        },
                    )
                    .unwrap();
                    let call = f
                        .runtime
                        .operations
                        .submit_endpoints::<QuerySchemas, _>(QuerySchemasRequest::default(), sender)
                        .unwrap();
                    assert!(receiver.recv().await.is_none());
                    assert!(call.await.is_err());
                    f.finish().await;
                });
        });
    }
}
