/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Run The Same Editing And Persistence Story With Either Plugin Backend.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::path::Path;

// Third Party Imports

// Local Imports
use crate::Result;
use harmos::{state::Focus, state::PrincipalId};
use schematic_app::domain::model;
use schematic_app::operations::{ProposeRouteRequest, ValidateSchematicRequest};
use schematic_app::state::{NetId, NetName, PinRef, Point, Rotation, SymbolId};
use schematic_app::transactions::net::{ApplyRoute, ConnectPins};
use schematic_app::transactions::symbol::{MoveSymbol, PlaceSymbol};
use schematic_app::{app, config::Settings, document::Document, services::Plugin, storage};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tracing::instrument(level = "debug", skip_all)]
pub async fn run(guest: Option<&Path>, sidecar: &Path, directory: &Path, advanced: bool) -> Result<()> {
    // Create A Document And Its Recovery Journal
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("schematic.json");
    let document = Document::new(&path);
    let recordings = scratch.path().join("streams");
    let config = Settings::new(recordings);
    let recovery = document.recovering()?;
    let checkpoint = recovery.sidecars();
    let origin = document.into_origin();
    let builder = app::builder(origin, ());
    let builder = storage::attach(builder, recovery, &config)?;
    let runtime = builder.runtime().await?;

    // Choose A Backend For The Same Operations
    let plugin = match guest {
        Some(path) => Plugin::guest(&runtime, path).await,
        None => Plugin::sidecar(&runtime, sidecar, directory).await,
    };
    let plugin = match plugin {
        Ok(plugin) => plugin,
        Err(error) => {
            runtime.stop().await?;
            return Err(error.into());
        }
    };

    let result = edit(&runtime, advanced, guest.is_none()).await;
    let stopped = plugin.stop().await;
    let snapshot = runtime.state.state().read(Clone::clone).await;
    let settled = runtime.stop().await;
    result?;
    stopped?;
    settled?;

    // Save And Reopen The Settled Document
    let snapshot = snapshot?;
    Document::save(&path, snapshot, &checkpoint)?;
    let document = Document::open(&path)?;
    let reopened = app::launch(document, &config).await?;
    let saved = reopened.state.state().read(Clone::clone).await;
    reopened.stop().await?;
    let saved = saved?;
    println!(
        "Reopened: {} symbols, {} net, {} route points",
        saved.value.symbols.len(),
        saved.value.nets.len(),
        saved.value.net(NetId(1)).route.len()
    );
    Ok(())
}

/* === Private Functions === */

async fn edit(runtime: &app::Runtime, advanced: bool, native: bool) -> Result<()> {
    let principal = PrincipalId::new("designer");
    let state = runtime.state.as_principal(principal.clone());

    // Place And Connect Two Symbols
    let resistor = PlaceSymbol { symbol: SymbolId(1), reference: "R1".to_owned(), at: Point::new(10, 20), rotation: Rotation::Zero };
    let controller = PlaceSymbol { symbol: SymbolId(2), reference: "U1".to_owned(), at: Point::new(60, 40), rotation: Rotation::Zero };
    state.commit(resistor).await?;
    state.commit(controller).await?;
    let connection = ConnectPins {
        net: NetId(1),
        name: NetName::verbatim("POWER"),
        pins: BTreeSet::from([PinRef::new(SymbolId(1), 1), PinRef::new(SymbolId(2), 1)]),
    };
    state.commit(connection).await?;
    let movement = MoveSymbol { symbol: SymbolId(1), to: Point::new(20, 20) };
    state.commit(movement).await?;
    println!("Placed R1 and U1, connected POWER, moved R1");

    // Ask The Plugin For Advice; The Host Owns Edits
    let snapshot = state.state().read(Clone::clone).await?.value;
    let symbols = snapshot.symbols.keys().map(|id| id.0).collect();
    let connections = snapshot
        .nets
        .values()
        .flat_map(|net| {
            let symbols: Vec<_> = net.pins.iter().map(|pin| pin.symbol.0).collect();
            symbols.windows(2).map(|pair| model::Connection { from: pair[0], to: pair[1] }).collect::<Vec<_>>()
        })
        .collect();
    let schematic = model::Snapshot { symbols, connections };
    let validation = ValidateSchematicRequest { schematic: schematic.clone() };
    let validation = runtime.operation(validation).run().await?;
    println!("Validation: {} diagnostics", validation.diagnostics.len());

    let from = snapshot.symbol(SymbolId(1)).position;
    let to = snapshot.symbol(SymbolId(2)).position;
    let request = ProposeRouteRequest { from: model::Point { x: from.x, y: from.y }, to: model::Point { x: to.x, y: to.y } };
    let proposed = runtime.operation(request).run().await?;
    let points = proposed.points.iter().map(|point| Point::new(point.x, point.y)).collect();
    let change = ApplyRoute { net: NetId(1), points };
    state.commit(change).await?;
    println!("Applied route: {:?}", proposed.points);

    // Undo And Redo The Host Transaction
    state.undo(principal.clone(), Focus::all()).await?;
    state.redo(principal, Focus::all()).await?;
    println!("Undid and redid the route");

    if advanced {
        crate::advanced::run(runtime, schematic, native).await?;
    }
    Ok(())
}

/* === Tests === */
