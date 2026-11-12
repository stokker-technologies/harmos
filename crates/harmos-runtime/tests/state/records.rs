/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos_runtime::{Error, Runtime, state::DecodeError, state::Position, state::PrincipalId, state::Recorded};

/* === Structs === */

#[derive(harmos_runtime::Described, Clone, Debug, Serialize, Deserialize)]
struct Noted(String);

struct Model;

/* === Enums === */

#[derive(Clone, Debug)]
enum Records {
    Noted(Noted),
}
impl From<Noted> for Records {
    fn from(value: Noted) -> Self {
        Self::Noted(value)
    }
}
impl harmos_runtime::state::CatalogCodec for Records {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Noted(_) => <Noted as harmos_runtime::state::Record>::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Noted(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
fn records_registry() -> harmos_runtime::state::RecordRegistry<Records> {
    harmos_runtime::state::RecordRegistry::new().entry(<Noted as harmos_runtime::state::Record>::ENTRY, Records::Noted, |value| {
        let Records::Noted(value) = value;
        Some(value)
    })
}

/* === Implementations === */

impl harmos_runtime::state::Record for Noted {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("manager.note");
}

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = Records;
}

/* === Private Functions === */

#[tokio::test]
async fn undeclared_record_refused() {
    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
    let refused = runtime.records.publish(Noted("absent".into())).await;
    assert!(matches!(refused,
        Err(Error::RecordDeclaration { source: DecodeError::UnknownDefinition { id } })
        if id.as_str() == "manager.note"
    ));
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn producers_share_order() {
    let runtime = crate::support::app::launch::<Model>((), (), |setup| {
        setup.settings().register(records_registry());
    })
    .await
    .unwrap();
    let first = runtime.records.publish(Noted("first".into())).await.unwrap();
    let producer = runtime.records.as_principal(PrincipalId::new("editor")).caused_by(first.position).clone();
    let second = producer.publish(Noted("second".into())).await.unwrap();
    assert_eq!(first.position, Position(1));
    assert_eq!(second.position, Position(2));
    let entries: Vec<_> = runtime.state.query::<Recorded<Model>>(Position::ORIGIN).await.map(Result::unwrap).collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].metadata.principal, Some(PrincipalId::new("editor")));
    assert_eq!(entries[1].metadata.causation, Some(harmos_runtime::state::CausationId(first.position)));
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn discovery_shares_producer() {
    use harmos_runtime::schema::Described;
    use harmos_runtime::storage::SchemaKey;
    use std::num::NonZeroU32;
    use std::sync::Arc;

    #[derive(harmos_runtime::Message, Clone, Debug, Default)]
    struct Message {
        value: String,
    }

    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
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
    definition.validate(&harmos_runtime::Message::encode(&message)).unwrap();
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
            .register(
                "plugin",
                schema.clone(),
                harmos_runtime::schema::graph::Schema {
                    representations: Default::default(),
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn record_registration_uses_runtime_schema_settings() {
    use harmos_runtime::schema::Described;
    use std::num::{NonZeroU32, NonZeroUsize};

    #[derive(harmos_runtime::Message, Clone, Debug, Default)]
    struct ConfiguredRow {
        value: String,
    }

    let defaults = harmos_runtime::config::SchemaSettings::default();
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(ConfiguredRow::DEFINITION, &defaults).unwrap();
    let key = harmos_runtime::storage::SchemaKey {
        payload_encoding: harmos_runtime::storage::PayloadEncoding::PostcardV1,
        id: harmos_runtime::Identifier::new("configured.record").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    };
    for accepted in [true, false] {
        let mut settings = defaults;
        if !accepted {
            settings.preparation_bytes = NonZeroUsize::new(1).unwrap();
        }
        let runtime = crate::support::app::launch::<Model>((), (), |setup| {
            setup.settings().schema_settings(settings);
        })
        .await
        .unwrap();
        let result = runtime.records.register("configured", key.clone(), graph.clone()).await;
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(runtime.records.definition(&key).is_some(), accepted);
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
}
