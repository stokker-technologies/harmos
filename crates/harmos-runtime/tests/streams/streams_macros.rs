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
use std::convert::Infallible;
use std::pin::Pin;

// Third Party Imports

// Local Imports
use harmos_runtime::streams::StreamRow as _;
use harmos_runtime::{Application, Runtime, state::At, state::Position, state::Transaction, streams::Item};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct State;

struct Studio;

#[derive(harmos_runtime::Row)]
#[serde(crate = "harmos_runtime::__private::serde")]
#[derive(Clone, Debug, Default, PartialEq, harmos_runtime::__private::serde::Serialize, harmos_runtime::__private::serde::Deserialize)]
struct PresentedSample {
    #[row(description = "Measured voltage", label = "Voltage", role = value, unit = "V", tags("electrical"), attributes(precision = 4u64))]
    voltage: f64,

    sample_index: u64,
}

/* === Enums === */

enum Change {}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::streams::StreamRow for PresentedSample {
    const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("presentation.samples", 8).label("Samples").description("Portable samples").tags(&["lab"]).attributes_json(r#"{"acme.owner":{"type":"text","value":"bench"},"config":{"type":"object","value":{"gain":{"type":"integer","value":"2"},"channels":{"type":"array","value":[{"type":"integer","value":"1"},{"type":"integer","value":"2"}]}}}}"#);
}

impl Transaction<State> for Change {
    type Error = Infallible;

    fn check(&self, _state: &State) -> Result<(), Self::Error> {
        match *self {}
    }

    fn apply(&self, _state: &mut State) {
        match *self {}
    }
}

impl harmos_runtime::Lifecycle for Studio {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Studio {
    type State = State;
    type Transaction = Change;
    type Record = ();
}

impl harmos_runtime::state::CatalogCodec for Change {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match *self {}
    }
    fn encode<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        match *self {}
    }
}

impl harmos_runtime::state::TransactionCatalog<State> for Change {}

/* === Public Functions === */

/* === Private Functions === */

async fn next<S: Clone + 'static>(watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin)) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

async fn studio() -> Runtime<Studio> {
    Runtime::<Studio>::launch_with(State, (), |setup| {
        setup.settings().register(streams::stream_definitions());
    })
    .await
    .expect("the generated stream catalog is honorable")
}

#[tokio::test]
async fn streams_publish_query() {
    let runtime = studio().await;
    let demand = runtime.streams.demand::<streams::Pulse>();

    assert_eq!(*demand.borrow(), 0);
    let mut watching = runtime.streams.watch::<streams::Pulse>();
    assert_eq!(*demand.borrow(), 1);

    runtime.streams.publish(streams::Pulse(7));
    runtime.streams.publish(streams::Pulse(8));

    assert_eq!(
        next(&mut watching).await,
        Some(Item::Row(At {
            value: streams::Pulse(7),
            position: Position::ORIGIN
        }))
    );
    assert_eq!(
        next(&mut watching).await,
        Some(Item::Row(At {
            value: streams::Pulse(8),
            position: Position::ORIGIN
        }))
    );

    let queried: Vec<_> = runtime
        .streams
        .query::<streams::Pulse>(..)
        .await
        .expect("the generated four-row window has not evicted")
        .map(|row| row.value.0)
        .collect();
    assert_eq!(queried, [7, 8]);

    drop(watching);
    assert_eq!(*demand.borrow(), 0);
    runtime.shutdown().await.unwrap();
}

mod streams {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Deserializer, Serialize};

    // Local Imports

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[derive(harmos_runtime::Row, Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    pub struct Pulse(pub u64);

    #[derive(harmos_runtime::Row, Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Reading {
        pub value: i64,
    }

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::streams::StreamRow for Pulse {
        const ENTRY: harmos_runtime::StreamEntry<Self> =
            harmos_runtime::StreamEntry::new("pulse", 4).label("Pulse").description("A waveform-shaped row.");
    }

    impl harmos_runtime::streams::StreamRow for Reading {
        const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("reading", 2)
            .label("Reading")
            .description("A second vertical row proves dispatch and demand stay per-stream.");
    }

    impl<'de> Deserialize<'de> for Pulse {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            u64::deserialize(deserializer).map(Self)
        }
    }

    /* === Public Functions === */

    pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
        vec![
            <Pulse as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap(),
            <Reading as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap(),
        ]
    }

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */

#[test]
fn stream_values_register() {
    let catalog = streams::stream_definitions();

    assert_eq!(catalog.iter().map(|definition| definition.id()).collect::<Vec<_>>(), ["pulse", "reading"]);
    assert_eq!(catalog[0].id(), "pulse");
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(
        <streams::Pulse as harmos_runtime::schema::Described>::DEFINITION,
        &harmos_runtime::config::SchemaSettings::default(),
    )
    .unwrap();
    assert_eq!(catalog[0].version(), harmos_domain::streams::revision(&graph, catalog[0].window()).unwrap());
    assert_eq!(catalog[0].window(), 4);
    assert_eq!(catalog.iter().find(|definition| definition.id() == "reading").expect("the id is in the catalog").window(), 2);
    assert!(catalog.iter().find(|definition| definition.id() == "outside").is_none());
    assert_eq!(streams::Pulse::definition().id(), "pulse");
    assert_eq!(<streams::Pulse as harmos_runtime::streams::StreamRow>::ID, "pulse");
}

#[test]
fn row_and_stream_values_preserve_presentation() {
    use harmos_runtime::{Message, StreamSpec};

    let row = PresentedSample { voltage: -12.5, sample_index: u64::MAX };
    let decoded = PresentedSample::decode(&row.encode()).unwrap();
    assert_eq!(decoded.voltage.to_bits(), row.voltage.to_bits());
    assert_eq!(decoded.sample_index, u64::MAX);
    assert_eq!(<PresentedSample as StreamSpec>::metadata().label, "Samples");
    assert_eq!(<PresentedSample as StreamSpec>::metadata().attributes["acme.owner"], harmos_runtime::Value::Text("bench".into()));
    assert!(matches!(<PresentedSample as StreamSpec>::metadata().attributes["config"], harmos_runtime::Value::Object(_)));
    assert_eq!(<PresentedSample as StreamSpec>::row_metadata()[0].unit.as_deref(), Some("V"));
    assert_eq!(<PresentedSample as StreamSpec>::row_metadata()[1].metadata.label, "Sample Index");
    assert_eq!(
        <PresentedSample as StreamSpec>::row_metadata()[0].metadata.description.as_deref(),
        Some("Measured voltage"),
        "a field's explicit description is retained"
    );
    assert_eq!(PresentedSample::definition().metadata().description.as_deref(), Some("Portable samples"));

    #[cfg(feature = "plugin-guest")]
    const ENTRIES: &[harmos_runtime::guest::inventory::publications::Entry] =
        &[harmos_runtime::guest::inventory::publications::Entry::of::<PresentedSample>()];
    #[cfg(feature = "plugin-guest")]
    assert_eq!(ENTRIES[0].metadata(), <PresentedSample as StreamSpec>::metadata());
    #[cfg(feature = "plugin-guest")]
    assert_eq!(ENTRIES[0].row_metadata(), <PresentedSample as StreamSpec>::row_metadata());
}

#[test]
fn stream_values_validate_identity_window_and_presentation() {
    let valid = harmos_runtime::StreamEntry::<PresentedSample>::new("values.samples", 4).label("Samples");
    assert_eq!(valid.build().unwrap().metadata().label, "Samples");
    assert!(matches!(
        harmos_runtime::StreamEntry::<PresentedSample>::new("bad/path", 4).build(),
        Err(harmos_domain::inventory::publications::Error::InvalidId { .. })
    ));
    assert!(matches!(
        harmos_runtime::StreamEntry::<PresentedSample>::new("values.samples", 0).build(),
        Err(harmos_domain::inventory::publications::Error::ZeroWindow { .. })
    ));
    assert!(matches!(valid.attributes_json("broken").build(), Err(harmos_domain::inventory::publications::Error::Metadata { .. })));
    assert!(matches!(valid.tags(&["duplicate", "duplicate"]).build(), Err(harmos_domain::inventory::publications::Error::Metadata { .. })));
}
