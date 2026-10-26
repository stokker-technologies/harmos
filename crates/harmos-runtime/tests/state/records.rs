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

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos_runtime::{Error, Runtime, state::DecodeError, state::Position, state::PrincipalId, state::Recorded};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::record(id = "manager.note", version = 1)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Noted(String);

struct Model;

/* === Enums === */

#[harmos_runtime::records]
#[derive(Clone, Debug)]
enum Records {
    Noted(Noted),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn undeclared_record_refused() {
    let runtime = Runtime::<Model>::builder((), ()).runtime().await.unwrap();
    let refused = runtime.records.publish(Noted("absent".into())).await;
    assert!(matches!(refused,
        Err(Error::RecordDeclaration { source: DecodeError::UnknownDefinition { id } })
        if id.as_str() == "manager.note"
    ));
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn producers_share_order() {
    let runtime = Runtime::<Model>::builder((), ()).register(Records::catalog()).runtime().await.unwrap();
    let first = runtime.records.publish(Noted("first".into())).await.unwrap();
    let producer = runtime.records.as_principal(PrincipalId::new("editor")).caused_by(first.position).clone();
    let second = producer.publish(Noted("second".into())).await.unwrap();
    assert_eq!(first.position, Position(1));
    assert_eq!(second.position, Position(2));
    let entries: Vec<_> = runtime.state.query::<Recorded<Model>>(Position::ORIGIN).await.map(Result::unwrap).collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].metadata.principal, Some(PrincipalId::new("editor")));
    assert_eq!(entries[1].metadata.causation, Some(harmos_runtime::state::CausationId(first.position)));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn discovery_shares_producer() {
    use harmos_runtime::schema::Described;
    use harmos_runtime::storage::SchemaKey;
    use std::num::NonZeroU32;
    use std::sync::Arc;

    #[harmos_runtime::message]
    struct Message {
        value: String,
    }

    let runtime = Runtime::<Model>::builder((), ()).runtime().await.unwrap();
    let schema = SchemaKey {
        payload_encoding: harmos_runtime::storage::PayloadEncoding::PostcardV1,
        id: harmos_runtime::Identifier::new("plugin.note").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    };
    let definition = runtime
        .records
        .register(
            "plugin",
            schema.clone(),
            harmos_runtime::schema::graph::Schema::from_declaration(
                Message::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let producer = harmos_runtime::records::RecordManager::new(runtime.state.as_principal(PrincipalId::new("editor")));
    assert!(Arc::ptr_eq(&definition, &producer.definition(&schema).unwrap()));
    assert!(Arc::ptr_eq(
        &definition,
        &producer
            .register(
                "plugin",
                schema.clone(),
                harmos_runtime::schema::graph::Schema::from_declaration(
                    Message::DEFINITION,
                    &harmos_runtime::config::SchemaSettings::default()
                )
                .unwrap()
            )
            .await
            .unwrap()
    ));
    let message = Message { value: "saved".into() };
    definition.validate(&harmos_runtime::Message::encode_to_vec(&message)).unwrap();
    assert!(matches!(definition.validate(&[0xff]), Err(Error::RecordPayload { .. })));
    assert!(
        producer
            .register(
                "other",
                schema.clone(),
                harmos_runtime::schema::graph::Schema::from_declaration(
                    Message::DEFINITION,
                    &harmos_runtime::config::SchemaSettings::default()
                )
                .unwrap()
            )
            .await
            .is_err()
    );
    assert!(
        producer
            .register("plugin", schema.clone(), harmos_runtime::schema::graph::Schema { root: 0, definitions: vec![vec![]] })
            .await
            .is_err()
    );
    assert_eq!(producer.definitions().len(), 1);
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    drop(producer);
    runtime.stop().await.unwrap();
}

/* === Tests === */
