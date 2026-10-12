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
use std::collections::BTreeSet;
use std::path::Path;
// Third Party Imports

// Local Imports
use crate::error::Error;
use harmos::{Message, Position, PrincipalId};
use minischematic_app::app::{self, Runtime};
use minischematic_app::config::Config;
use minischematic_app::document::Document;
use minischematic_app::services::autoroute;
use minischematic_app::state::{NetId, NetName, PinRef, Point, Rotation, Schematic, SymbolId};
use minischematic_app::transactions::Transactions;
use minischematic_app::transactions::net::ConnectPins;
use minischematic_app::transactions::symbol::PlaceSymbol;

/* === Constants === */
const R1: SymbolId = SymbolId(1);
const U3: SymbolId = SymbolId(2);
const POWER: NetId = NetId(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(component: &Path, config: &Config) -> Result<String, Error> {
    let directory = tempfile::tempdir()?;
    let document = Document::new(directory.path().join("schematic.json"));
    let runtime = app::launch(document.clone(), config).await?;
    seed(&runtime).await;

    // Inspect The Component Before Collecting Its Bounded Metadata And SDK Loans.
    let bytes = std::fs::read(component)?;
    let loaded = autoroute::load(&runtime, &bytes)?;
    assert_eq!(loaded.declaration().id, autoroute::NAME);
    assert_eq!(
        loaded
            .declaration()
            .streams
            .iter()
            .map(|stream| stream.id.as_str())
            .collect::<Vec<_>>(),
        ["progress"]
    );

    autoroute::register_host(&runtime)?;
    let (mut manager, mut lifecycle) = autoroute::lifecycle(&runtime, loaded).await?;
    manager.initialize(&mut lifecycle).await?;
    let routed = whole(&runtime).await;
    assert_eq!(routed.net(POWER).name, NetName::parse("VCC"));

    let changes: Vec<_> = runtime
        .state
        .query::<Transactions>(Position::ORIGIN)
        .await
        .collect::<Result<_, _>>()?;
    let route = changes
        .last()
        .expect("Autoroute committed one application change");
    assert_eq!(
        route.metadata.principal.as_ref().map(PrincipalId::as_str),
        Some("guest:autoroute")
    );
    assert_eq!(
        route.metadata.request.as_ref().map(|key| key.as_str()),
        Some("route-vcc")
    );

    let progress: Vec<u32> = runtime
        .streams
        .publication(autoroute::Progress::ID, autoroute::Progress::VERSION)
        .expect("the inspected guest declares progress")
        .query(..)
        .await?
        .map(|publication| {
            autoroute::Progress::decode(publication.value.payload.as_slice()).map(|row| row.percent)
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(progress, [10, 100]);

    manager.shutdown(&mut lifecycle).await?;
    drop(manager);
    drop(lifecycle);
    runtime.stop().await?;

    let recovered = app::launch(document, config).await?;
    assert_eq!(whole(&recovered).await, routed);
    recovered.stop().await?;

    Ok(concat!(
        "1  open -> ordinary schematic with one unnamed power net\n",
        "2  inspect -> Autoroute metadata read without executing Wasm\n",
        "3  load -> component compiled only after static validation\n",
        "4  initialize -> sealed rename invocation committed once as guest:autoroute\n",
        "5  progress -> declared publications observed at 10% and 100%\n",
        "6  reopen without the guest -> VCC recovers unchanged\n",
    )
    .to_owned())
}

/* === Private Functions === */
async fn seed(runtime: &Runtime) {
    let editor = runtime.state.as_principal(PrincipalId::new("alice"));
    for (symbol, reference, x) in [(R1, "R1", 10), (U3, "U3", 60)] {
        editor
            .commit(PlaceSymbol {
                symbol,
                reference: reference.to_owned(),
                at: Point::new(x, 20),
                rotation: Rotation::Zero,
            })
            .await
            .expect("the teaching schematic starts empty");
    }
    editor
        .commit(ConnectPins {
            net: POWER,
            name: NetName::verbatim("N$1"),
            pins: BTreeSet::from([PinRef::new(R1, 1), PinRef::new(U3, 7)]),
        })
        .await
        .expect("both teaching symbols are placed");
}

async fn whole(runtime: &Runtime) -> Schematic {
    runtime
        .state
        .state()
        .read(Clone::clone)
        .await
        .expect("the schematic remains readable")
        .value
}

/* === Tests === */
