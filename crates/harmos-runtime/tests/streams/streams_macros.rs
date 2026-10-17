/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod streams {
    use serde::{Deserialize, Deserializer, Serialize};

    #[harmos_runtime::stream]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PulseV1(pub u64);

    impl<'de> Deserialize<'de> for PulseV1 {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            u64::deserialize(deserializer).map(Self)
        }
    }

    #[harmos_runtime::stream(
        description = "A waveform-shaped row with one historical schema.",
        id = "pulse",
        window = 4,
        history(PulseV1)
    )]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    pub struct Pulse(pub u64);

    impl<'de> Deserialize<'de> for Pulse {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            u64::deserialize(deserializer).map(Self)
        }
    }

    impl TryFrom<PulseV1> for Pulse {
        type Error = String;

        fn try_from(historical: PulseV1) -> Result<Self, Self::Error> {
            Ok(Self(historical.0 + 1))
        }
    }

    #[harmos_runtime::stream(
        description = "A second vertical row proves dispatch and demand stay per-stream.",
        id = "reading",
        window = 2
    )]
    #[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Reading {
        pub value: i64,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
    pub struct Take(pub u64);

    impl harmos_runtime::streams::Track for Take {
        fn label(&self) -> String {
            format!("take_{}", self.0)
        }
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
    pub struct Lens(pub u64);

    impl harmos_runtime::streams::Track for Lens {
        fn label(&self) -> String {
            format!("lens_{}", self.0)
        }
    }

    #[harmos_runtime::stream(description = "A row declaring both optional coordinates.",
        id = "capture",
        window = 4,
        track = (take, lens),
        axis = ordinal
    )]
    #[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Capture {
        pub take: Take,
        pub lens: Lens,
        pub ordinal: u64,
        pub value: i64,
    }

    #[harmos_runtime::streams]
    pub enum Studio {
        Pulse(Pulse),
        Reading(Reading),
        Capture(Capture),
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
use std::pin::Pin;
// Third Party Imports
use serde::de::value::{Error as ValueError, U64Deserializer};
// Local Imports
use harmos_runtime::{
    Application, Runtime, state::At, state::Position, state::Transaction, streams::Axial,
    streams::Item, streams::Prefix, streams::Stream, streams::Tracked,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct State;

struct Studio;

#[harmos_runtime::stream(description = "Portable samples", id = "presentation.samples", label = "Samples", window = 8, tags("lab"), attributes("acme.owner" = "bench", config = {"gain": 2, channels: [1, 2]}))]
struct PresentedSample {
    #[row(description = "Measured voltage", label = "Voltage", role = value, unit = "V", tags("electrical"), attributes(precision = 4u64))]
    voltage: f64,
    sample_index: u64,
}

/* === Enums === */
enum Change {}

/* === Traits === */

/* === Implementations === */
impl Transaction<State> for Change {
    type Error = Infallible;

    fn check(&self, _state: &State) -> Result<(), Self::Error> {
        match *self {}
    }

    fn apply(&self, _state: &mut State) {
        match *self {}
    }
}

impl Application for Studio {
    type State = State;
    type Transaction = Change;
    type Record = ();
    type Streams = streams::Studio;
    type Resources = ();
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
async fn next<S: Clone + 'static>(
    watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin),
) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

async fn studio() -> Runtime<Studio> {
    Runtime::<Studio>::builder(State, ())
        .register(streams::Studio::catalog())
        .runtime()
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
            position: Position::ORIGIN,
        }))
    );
    assert_eq!(
        next(&mut watching).await,
        Some(Item::Row(At {
            value: streams::Pulse(8),
            position: Position::ORIGIN,
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
    runtime.stop().await.unwrap();
}

#[test]
fn generated_catalog_registers() {
    let catalog = streams::Studio::catalog();

    assert_eq!(catalog.len(), 3, "one enum variant yields one definition");
    assert_eq!(catalog[0].id(), "pulse");
    assert_eq!(catalog[0].version(), 2);
    assert_eq!(catalog[0].window(), 4);
    assert_eq!(
        streams::Studio::definition("reading")
            .expect("the id is in the catalog")
            .window(),
        2
    );
    assert!(streams::Studio::definition("outside").is_none());
    assert_eq!(streams::Pulse::DEFINITION, ("pulse", 2, 4));
    assert_eq!(streams::Pulse::HISTORY, [("pulse", 1)]);
    assert_eq!(<streams::Pulse as Stream<Studio>>::ID, "pulse");
}

#[test]
fn declared_coordinates_carry() {
    let row = streams::Capture {
        take: streams::Take(3),
        lens: streams::Lens(9),
        ordinal: 41,
        value: -7,
    };

    assert_eq!(
        row.track(),
        (streams::Take(3), streams::Lens(9)),
        "the declaration's depth and component types are the row's own"
    );
    assert_eq!(<streams::Capture as Tracked>::TRACK, ["take", "lens"]);
    assert_eq!(row.axis(), 41);
    assert_eq!(<streams::Capture as Axial>::AXIS, "ordinal");
}

#[test]
fn selection_prefix_declared() {
    fn narrows<S: Tracked, K: Prefix<S::Track>>(selection: K) -> Vec<String> {
        selection.labels()
    }

    assert_eq!(
        narrows::<streams::Capture, _>((streams::Take(3),)),
        ["take_3"],
        "a shorter tuple selects the whole subtree beneath it"
    );
    assert_eq!(
        narrows::<streams::Capture, _>((streams::Take(3), streams::Lens(9))),
        ["take_3", "lens_9"],
        "the whole tuple selects one partition, and its labels are its components'"
    );
    assert!(
        (streams::Take(3),).selects(&(streams::Take(3), streams::Lens(1))),
        "a prefix selects by the application's own equality, component by component"
    );
    assert!(
        !(streams::Take(3),).selects(&(streams::Take(4), streams::Lens(1))),
        "and cuts everything it does not name"
    );
    // Neither of these implements `Prefix<(Take, Lens)>`, so neither compiles:
    //
    //     narrows::<streams::Capture, _>((streams::Lens(9),));
    //     narrows::<streams::Capture, _>((streams::Take(3), streams::Lens(9), streams::Take(1)));
}

#[test]
fn rows_restore_direct() {
    assert_eq!(
        streams::Pulse::restore(1, U64Deserializer::<ValueError>::new(40))
            .expect("the historical row decodes")
            .expect("version 1 is declared"),
        streams::Pulse(41),
        "the historical row converts directly into the current row"
    );
    assert_eq!(
        streams::Pulse::restore(2, U64Deserializer::<ValueError>::new(9))
            .expect("the current row decodes"),
        Some(streams::Pulse(9))
    );
    assert!(
        streams::Pulse::restore(99, U64Deserializer::<ValueError>::new(9))
            .expect("an unknown version is not a decode error")
            .is_none()
    );
}

#[test]
fn stream_attribute_carries() {
    use harmos_runtime::{Message, StreamSpec};

    let row = PresentedSample {
        voltage: -12.5,
        sample_index: u64::MAX,
    };
    let decoded = PresentedSample::decode(&row.encode_to_vec()).unwrap();
    assert_eq!(decoded.voltage.to_bits(), row.voltage.to_bits());
    assert_eq!(decoded.sample_index, u64::MAX);
    assert_eq!(<PresentedSample as StreamSpec>::metadata().label, "Samples");
    assert_eq!(
        <PresentedSample as StreamSpec>::metadata().attributes["acme.owner"],
        harmos_runtime::Value::Text("bench".into())
    );
    assert!(matches!(
        <PresentedSample as StreamSpec>::metadata().attributes["config"],
        harmos_runtime::Value::Object(_)
    ));
    assert_eq!(
        <PresentedSample as StreamSpec>::row_metadata()[0]
            .unit
            .as_deref(),
        Some("V")
    );
    assert_eq!(
        <PresentedSample as StreamSpec>::row_metadata()[1]
            .metadata
            .label,
        "Sample Index"
    );
    assert_eq!(
        <PresentedSample as StreamSpec>::row_metadata()[0]
            .metadata
            .description
            .as_deref(),
        Some("Measured voltage"),
        "a field's explicit description is retained"
    );
    assert_eq!(
        PresentedSample::definition()
            .metadata()
            .description
            .as_deref(),
        Some("Portable samples")
    );

    #[cfg(feature = "plugin")]
    const ENTRIES: &[harmos_runtime::plugin::inventory::publications::Entry] =
        &[harmos_runtime::plugin::inventory::publications::Entry::of::<
            PresentedSample,
        >()];
    #[cfg(feature = "plugin")]
    assert_eq!(
        ENTRIES[0].metadata(),
        <PresentedSample as StreamSpec>::metadata()
    );
    #[cfg(feature = "plugin")]
    assert_eq!(
        ENTRIES[0].row_metadata(),
        <PresentedSample as StreamSpec>::row_metadata()
    );
}

/* === Tests === */
