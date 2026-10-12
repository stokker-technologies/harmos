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
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, Operations, config};
use harmos::Message;
use harmos::load::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition,
    PluginDefinitions, PluginEndpoint, PluginLifecycle, PluginLifecycleAuthority, PluginManager,
    PluginTransition, Router,
};
use harmos::operations::{OperationManager, WireDeclaration, WireOperation};
use harmos::storage::frame::{Expr, Frame, GroupBy, ScalarField, ScalarPath};
use harmos::storage::operations::{
    Acknowledge, AcknowledgeRequest, Reductions, ReductionsRequest, WriteRecords,
    WriteRecordsRequest,
};
use harmos::storage::reduce::{Agg, ScalarKind, ScalarValue};
use harmos::storage::{MutationId, RecordInput};
use harmos::storage::{ReadKind, ReadLimits, ReadSelection, SchemaKey};
use harmos::{Error, Identifier, Result};
use harmos_contract::inventory::contracts::Required;
use harmos_contract::plugin::Grant;

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
    O: WireOperation,
    O::Request: harmos::schema::Described,
    O::Response: harmos::schema::Described,
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

fn write(fixture: &Fixture, key: &str, source: &str) -> WriteRecordsRequest {
    WriteRecordsRequest {
        mutation: MutationId {
            window: fixture.storage.mutation_window().unwrap().id,
            key: Identifier::new(key).unwrap(),
        },
        records: vec![RecordInput {
            schema: super::super::schema(),
            scope: None,
            item: Identifier::new("reading").unwrap(),
            source: None,
            source_time: None,
            payload: b"reading".to_vec(),
        }],
        source: Identifier::new(source).unwrap(),
    }
}

fn frame(schemas: &[SchemaKey], max_groups: usize) -> ReductionsRequest {
    let mut frame = Frame::new(
        NonZeroUsize::new(64).unwrap(),
        NonZeroUsize::new(max_groups).unwrap(),
    );
    for schema in schemas {
        let mut selection = ReadSelection::new(ReadKind::Records);
        selection.schema = Some(schema.clone());
        let scan = frame.scan(
            selection,
            vec![ScalarField {
                path: ScalarPath::Payload(vec![Identifier::new("value").unwrap()]),
                kind: ScalarKind::Unsigned,
            }],
        );
        let group_by = GroupBy::new(scan)
            .agg(Expr::Col(0), Agg::Mean)
            .agg(Expr::Col(0), Agg::Len);
        frame.group_by(if max_groups > 1 {
            group_by.keys(vec![0]).agg(Expr::Col(0), Agg::Median)
        } else {
            group_by
        });
    }
    ReductionsRequest {
        frame,
        page: ReadLimits {
            rows: NonZeroUsize::new(16).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        },
    }
}

async fn seed(fixture: &Fixture, values: &[u64]) {
    let field = harmos::schema::Field {
        name: "value".into(),
        tag: 1,
        kind: harmos::schema::Kind::U64,
        cardinality: harmos::schema::Cardinality::Required,
    };
    let schema = super::super::schema();
    fixture
        .storage
        .write_schema(&schema, &[field])
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
                    Operations::register(
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
                                    required::<WriteRecords>(vec![Grant::Source("bench".into())]),
                                    required::<Acknowledge>(Vec::new()),
                                ],
                            ),
                            plugin("bystander", Vec::new()),
                        ],
                        [],
                    )
                    .unwrap();
                    assert_eq!(definitions.host_requirements().count(), 2);
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

                    // The Granted Source Appends; Another Source Or An Ungranted Caller Is Refused
                    let cursor = writer
                        .submit::<WriteRecords>(write(&fixture, "first", "bench"))
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(matches!(
                        writer.submit::<WriteRecords>(write(&fixture, "second", "other")),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert!(matches!(
                        bystander.submit::<WriteRecords>(write(&fixture, "third", "bench")),
                        Err(Error::OperationUnauthorized { .. })
                    ));

                    // A Receipt Binds To The Calling Plugin And Its Generation
                    let receipt = writer
                        .submit::<Acknowledge>(AcknowledgeRequest {
                            replica: Identifier::new("mirror").unwrap(),
                            cursor: (*cursor).clone(),
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(*receipt, *cursor);
                    let receipts = fixture.storage.receipts();
                    assert_eq!(receipts.len(), 1);
                    assert_eq!(receipts[0].holder.owner, "writer");
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .submit::<Acknowledge>(AcknowledgeRequest {
                                replica: Identifier::new("mirror").unwrap(),
                                cursor: (*cursor).clone(),
                            })
                            .unwrap()
                            .await
                            .is_err(),
                        "the host itself holds no replica"
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reductions_collect_frames", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    seed(&fixture, &[9_007_199_254_740_993, 9_007_199_254_740_995]).await;
                    Operations::register(
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
                    let survey = |id: &str| Grant::Survey(id.to_owned().into());
                    let definitions = PluginDefinitions::new(
                        [
                            plugin(
                                "reducer",
                                vec![required::<Reductions>(vec![survey(schema.id.as_str())])],
                            ),
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

                    // The Granted Schema Collects Exactly, As The In-Process Query Does
                    let collected = reducer
                        .submit::<Reductions>(frame(std::slice::from_ref(&schema), 1))
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(
                        collected.outputs[0].groups[0].values,
                        vec![
                            ScalarValue::Unsigned(9_007_199_254_740_994),
                            ScalarValue::Unsigned(2)
                        ]
                    );

                    // A Second Schema Widens The Grant To Every Schema; Nobody Else Reads
                    assert!(matches!(
                        reducer.submit::<Reductions>(frame(&[schema.clone(), other], 1)),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert!(matches!(
                        bystander.submit::<Reductions>(frame(std::slice::from_ref(&schema), 1)),
                        Err(Error::OperationUnauthorized { .. })
                    ));

                    // Explained Memory Beyond The View's Working Allowance Is Refused Up Front
                    let mut oversized = frame(std::slice::from_ref(&schema), 4096);
                    oversized.frame.max_rows = NonZeroUsize::new(1 << 20).unwrap();
                    let refused = reducer
                        .submit::<Reductions>(oversized)
                        .unwrap()
                        .await
                        .unwrap_err();
                    assert!(
                        refused
                            .to_string()
                            .contains("explained frame memory exceeds query working memory"),
                        "{refused}"
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    fixture.finish().await;
                });
        });
    }
}
