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
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, config};
use harmos::Aggregate;
use harmos::Message;
use harmos::operations::{OperationDeclaration, OperationManager, WireDeclaration};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition,
    PluginDefinitions, PluginEndpoint, PluginLifecycle, PluginLifecycleAuthority, PluginManager,
    PluginTransition, Router,
};
use harmos::storage::operations::{Query, WriteRecords};
use harmos::storage::query::transform::{
    Transform,
    aggregate::{Aggregate as AggregateTransform, Measure},
};
use harmos::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};

use harmos::storage::SchemaKey;
use harmos::storage::writes::WriteReceipt;
use harmos::{Error, Identifier, Result};
use harmos_domain::inventory::contracts::Required;
use harmos_domain::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<(), (), Option<PluginEndpoint>>;

/* === Structs === */
struct Host {
    operations: OperationManager,
    callers: BTreeMap<String, PluginCaller>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl PluginLifecycle for Host {
    type Guest = ();
    type Sidecar = ();
    type Embedded = Option<PluginEndpoint>;

    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        self.operations.validate_requirements(candidate)
    }

    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(None))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let endpoint = router.enter(&definition.id).unwrap();
        self.callers
            .insert(definition.id.clone(), endpoint.caller());
        if let PluginBackend::Embedded(slot) = backend {
            *slot = Some(endpoint);
        }
        Ok(())
    }

    async fn stop(
        _: &PluginDefinition,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        if let PluginBackend::Embedded(slot) = backend {
            slot.take();
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn required<O>(grants: Vec<Grant>) -> Required
where
    O: OperationDeclaration,
{
    Required {
        contract: WireDeclaration::of::<O>().contract().unwrap(),
        grants,
    }
}

fn plugin(id: &str, required: Vec<Required>) -> PluginDefinition {
    PluginDefinition {
        id: id.into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded {
            revision: "fixture".into(),
        },
        requires: Vec::new(),
        configuration: b"null".to_vec(),
        contributions: PluginContributions {
            required_contracts: Some(required),
            ..PluginContributions::default()
        },
    }
}

fn query(fixture: &Fixture, schema: SchemaKey) -> Query {
    Query {
        at: fixture.storage.cursor_saved(),
        source: Source::Records(RecordSelection {
            schema,
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![Transform::Aggregate(AggregateTransform {
            groups: vec![],
            measures: vec![Measure {
                field: "payload.value".into(),
                statistic: Aggregate::Count,
                name: "count".into(),
            }],
        })],
        slice: RowSlice {
            offset: 0,
            limit: None,
        },
    }
}

async fn seed(fixture: &Fixture, values: &[u64]) {
    let field = harmos::schema::graph::Field {
        name: "value".into(),
        tag: 1,
        kind: harmos::schema::graph::Kind::U64,
        cardinality: harmos::schema::Cardinality::Required,
    };
    let schema = super::super::schema();
    fixture
        .storage
        .write_schema(
            &schema,
            &harmos::schema::graph::Schema {
                root: 0,
                definitions: vec![vec![field]],
            },
        )
        .await
        .unwrap();
    for (index, value) in values.iter().enumerate() {
        let payload = value.encode_to_vec();
        let item = Identifier::new(format!("value{index}")).unwrap();
        fixture
            .storage
            .write_records(&[super::super::record(&schema, &item, &payload)])
            .await
            .unwrap();
    }
    fixture.storage.flush().await.unwrap();
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("requirements_route_plugins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    harmos::storage::operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    let mut host = Host {
                        operations: fixture.runtime.operations.clone(),
                        callers: BTreeMap::new(),
                    };
                    let definitions = PluginDefinitions::new(
                        [
                            plugin(
                                "writer",
                                vec![
                                    required::<WriteRecords>(vec![Grant::of(
                                        WriteRecords::ID,
                                        "bench",
                                    )]),
                                ],
                            ),
                            plugin("bystander", Vec::new()),
                        ],
                        [],
                    )
                    .unwrap();
                    assert_eq!(definitions.host_requirements().count(), 1);
                    let mut manager = PluginManager::new(definitions).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let writer = fixture
                        .runtime
                        .operations
                        .caller(&host.callers["writer"])
                        .unwrap();
                    let bystander = fixture
                        .runtime
                        .operations
                        .caller(&host.callers["bystander"])
                        .unwrap();

                    super::writes::seed(&fixture).await;
                    let mut exchange = writer
                        .exchange::<WriteRecords>(
                            WriteRecords { scope: None },
                            super::writes::settings(),
                            super::writes::settings(),
                        )
                        .unwrap();
                    exchange
                        .send(super::writes::batch(1, "bench"))
                        .await
                        .unwrap();
                    let cursor = loop {
                        let receipt = exchange.next().await.unwrap().unwrap();
                        if let WriteReceipt::Durable(receipt) = &*receipt {
                            break Arc::new(receipt.cursor.clone());
                        }
                    };
                    // A Second Destination On The Same Invocation Needs Its Own Grant.
                    exchange
                        .send(super::writes::batch(2, "other"))
                        .await
                        .unwrap();
                    exchange.finish_input();
                    while exchange.next().await.unwrap().is_some() {}
                    let failure = exchange.join().await.unwrap_err();
                    assert!(matches!(
                        &failure,
                        Error::OperationFailed { source, .. }
                            if matches!(source.downcast_ref::<Error>(), Some(Error::OperationUnauthorized { .. }))
                    ), "unexpected write failure: {failure:?}");
                    assert_eq!(fixture.data.records.lock().unwrap().len(), 2);
                    assert_eq!(fixture.storage.cursor_accepted(), *cursor);
                    assert!(matches!(
                        bystander.exchange::<WriteRecords>(
                            WriteRecords { scope: None },
                            super::writes::settings(),
                            super::writes::settings()
                        ),
                        Err(Error::OperationUnauthorized { .. })
                    ));

                    manager.shutdown(&mut host).await.unwrap();
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("query_enforces_grants", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    seed(&fixture, &[9_007_199_254_740_993, 9_007_199_254_740_995]).await;
                    harmos::storage::operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    let mut host = Host {
                        operations: fixture.runtime.operations.clone(),
                        callers: BTreeMap::new(),
                    };
                    let schema = super::super::schema();
                    let other = SchemaKey {
                        id: Identifier::new("other").unwrap(),
                        version: NonZeroU32::MIN,
                        ..schema.clone()
                    };
                    let survey = |schema: &SchemaKey| {
                        Grant::of(
                            Query::ID,
                            query(&fixture, schema.clone()).source.grant_scope(),
                        )
                    };
                    let definitions = PluginDefinitions::new(
                        [
                            plugin("reducer", vec![required::<Query>(vec![survey(&schema)])]),
                            plugin("bystander", Vec::new()),
                        ],
                        [],
                    )
                    .unwrap();
                    let mut manager = PluginManager::new(definitions).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let reducer = fixture
                        .runtime
                        .operations
                        .caller(&host.callers["reducer"])
                        .unwrap();
                    let bystander = fixture
                        .runtime
                        .operations
                        .caller(&host.callers["bystander"])
                        .unwrap();

                    let mut collected = reducer
                        .download::<Query>(
                            query(&fixture, schema.clone()),
                            super::writes::settings(),
                        )
                        .unwrap();
                    let mut count = None;
                    while let Some(output) = collected.next().await.unwrap() {
                        if let QueryArrow::Batch(batch) = &QueryArrow::try_from(&*output).unwrap() {
                            let values = batch
                                .as_record_batch()
                                .column_by_name("count")
                                .unwrap()
                                .as_any()
                                .downcast_ref::<arrow_array::UInt64Array>()
                                .unwrap();
                            count = Some(values.value(0));
                        }
                    }
                    assert_eq!(count, Some(2));
                    assert_eq!(collected.join().await.unwrap().rows, 1);
                    let mut cross_kind = query(&fixture, schema.clone());
                    cross_kind.source = Source::Stream(harmos::storage::query::StreamSelection {
                        schema: schema.clone(),
                        scope: None,
                        item: None,
                        stream: None,
                        source_start: None,
                        source_end: None,
                        after: 0,
                    });
                    assert!(matches!(
                        reducer.download::<Query>(cross_kind, super::writes::settings()),
                        Err(Error::OperationUnauthorized { .. })
                    ));

                    assert!(matches!(
                        reducer
                            .download::<Query>(query(&fixture, other), super::writes::settings()),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert!(matches!(
                        bystander
                            .download::<Query>(query(&fixture, schema), super::writes::settings()),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    manager.shutdown(&mut host).await.unwrap();
                    fixture.finish().await;
                });
        });
    }
}
