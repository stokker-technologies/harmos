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

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::{
    Error, Runtime, state::DecodeError, state::Position, state::PrincipalId, state::Recorded,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::record(id = "manager.note", version = 1)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Noted(String);

struct Model;

/* === Enums === */
#[harmos::records]
#[derive(Clone, Debug)]
enum Records {
    Noted(Noted),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Model {
    type State = ();
    type Transaction = harmos::state::Never;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("consume_lacks_position", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = Runtime::<Model>::builder((), ()).runtime().await.unwrap();
    let refused = runtime.records.publish(Noted("absent".into())).await;
    assert!(matches!(refused,
        Err(Error::RecordDeclaration { source: DecodeError::UnknownDefinition { id } })
        if id.as_str() == "manager.note"
    ));
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("producers_share_order", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Model>::builder((), ())
                        .register(Records::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    let first = runtime
                        .records
                        .publish(Noted("first".into()))
                        .await
                        .unwrap();
                    let producer = runtime
                        .records
                        .as_principal(PrincipalId::new("editor"))
                        .caused_by(first.position)
                        .clone();
                    let second = producer.publish(Noted("second".into())).await.unwrap();
                    assert_eq!(first.position, Position(1));
                    assert_eq!(second.position, Position(2));
                    let entries: Vec<_> = runtime
                        .state
                        .query::<Recorded<Model>>(Position::ORIGIN)
                        .await
                        .map(Result::unwrap)
                        .collect();
                    assert_eq!(entries.len(), 2);
                    assert_eq!(
                        entries[1].metadata.principal,
                        Some(PrincipalId::new("editor"))
                    );
                    assert_eq!(
                        entries[1].metadata.causation,
                        Some(harmos::state::CausationId(first.position))
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("discovery_shares_producer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::schema::Described;
                    use harmos::storage::SchemaKey;
                    use std::num::NonZeroU32;
                    use std::sync::Arc;

                    #[harmos::message]
                    struct Message {
                        value: String,
                    }

                    let runtime = Runtime::<Model>::builder((), ()).runtime().await.unwrap();
                    let schema = SchemaKey {
                        payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
                        id: harmos::Identifier::new("plugin.note").unwrap(),
                        version: NonZeroU32::new(1).unwrap(),
                    };
                    let definition = runtime
                        .records
                        .register(
                            "plugin",
                            &schema,
                            &harmos::schema::graph::Schema::from_declaration(
                                Message::DEFINITION,
                                &harmos::config::SchemaSettings::default(),
                            )
                            .unwrap(),
                        )
                        .await
                        .unwrap();
                    let producer = harmos::records::RecordManager::new(
                        runtime.state.as_principal(PrincipalId::new("editor")),
                    );
                    assert!(Arc::ptr_eq(
                        &definition,
                        &producer.definition(&schema).unwrap()
                    ));
                    assert!(Arc::ptr_eq(
                        &definition,
                        &producer
                            .register(
                                "plugin",
                                &schema,
                                &harmos::schema::graph::Schema::from_declaration(
                                    Message::DEFINITION,
                                    &harmos::config::SchemaSettings::default()
                                )
                                .unwrap()
                            )
                            .await
                            .unwrap()
                    ));
                    let message = Message {
                        value: "saved".into(),
                    };
                    definition
                        .validate(&harmos::Message::encode_to_vec(&message))
                        .unwrap();
                    assert!(matches!(
                        definition.validate(&[0xff]),
                        Err(Error::RecordPayload { .. })
                    ));
                    assert!(
                        producer
                            .register(
                                "other",
                                &schema,
                                &harmos::schema::graph::Schema::from_declaration(
                                    Message::DEFINITION,
                                    &harmos::config::SchemaSettings::default()
                                )
                                .unwrap()
                            )
                            .await
                            .is_err()
                    );
                    assert!(
                        producer
                            .register(
                                "plugin",
                                &schema,
                                &harmos::schema::graph::Schema {
                                    root: 0,
                                    definitions: vec![vec![]]
                                }
                            )
                            .await
                            .is_err()
                    );
                    assert_eq!(producer.definitions().len(), 1);
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    drop(producer);
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
