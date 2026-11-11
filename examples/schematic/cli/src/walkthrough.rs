/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Run the Same Editing and Persistence Story with Either Plugin Backend.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::path::Path;

// Third Party Imports

// Local Imports
use crate::Result;
use harmos_runtime::{state::Focus, state::PrincipalId};
use schematic_app::domain::model;
use schematic_app::operations::{ProposeRoute, ValidateSchematic};
use schematic_app::state::{NetId, NetName, PinRef, Point, Rotation, SymbolId};
use schematic_app::transactions::net::{ApplyRoute, ConnectPins};
use schematic_app::transactions::symbol::{MoveSymbol, PlaceSymbol};
use schematic_app::{app, config::Settings, document::Document, services::Plugin};

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
    // Create a Document and Its Recovery Journal
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("schematic.json");
    let document = Document::new(&path);
    let recordings = scratch.path().join("streams");
    let config = Settings::new(recordings);
    let runtime = app::launch(document, &config).await?;
    let checkpoint = runtime.operations.context().resource::<schematic_app::storage::Sidecar>().map_err(std::io::Error::other)?;

    // Choose a Backend for the Same Operations
    let plugin = async {
        // Component Compilation Precedes the Bounded Plugin Initialization.
        let loaded = match guest {
            Some(path) => {
                let bytes = std::fs::read(path)?;
                Some(schematic_app::services::guest::load(&runtime, &bytes)?)
            }
            None => None,
        };
        eprintln!("schematic.plugin.initializing");
        let plugin = match loaded {
            Some(loaded) => Plugin::guest(&runtime, loaded).await?,
            None => Plugin::sidecar(&runtime, sidecar, directory).await?,
        };
        eprintln!("schematic.plugin.ready");
        Ok::<_, crate::Error>(plugin)
    }
    .await;
    let plugin = match plugin {
        Ok(plugin) => plugin,
        Err(error) => {
            runtime.shutdown().await?;
            return Err(error);
        }
    };

    // === Edit the Document === //
    let result = async {
        // Select the Designer
        let principal = PrincipalId::new("designer");
        let state = runtime.state.as_principal(principal.clone());

        // Place the Resistor
        let request = PlaceSymbol {
            symbol: SymbolId(1),
            reference: "R1".to_owned(),
            at: Point::new(10, 20),
            rotation: Rotation::Zero,
        };
        state.commit(request).await?;

        // Place the Controller
        let request = PlaceSymbol {
            symbol: SymbolId(2),
            reference: "U1".to_owned(),
            at: Point::new(60, 40),
            rotation: Rotation::Zero,
        };
        state.commit(request).await?;

        // Connect the Symbols
        let request = ConnectPins {
            net: NetId(1),
            name: NetName::verbatim("POWER"),
            pins: BTreeSet::from([PinRef::new(SymbolId(1), 1), PinRef::new(SymbolId(2), 1)]),
        };
        state.commit(request).await?;

        // Move the Resistor
        let request = MoveSymbol {
            symbol: SymbolId(1),
            to: Point::new(20, 20),
        };
        state.commit(request).await?;
        println!("Placed R1 and U1, connected POWER, moved R1");

        // Capture the Schematic
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

        // Validate the Schematic
        let request = ValidateSchematic { schematic: schematic.clone() };
        let diagnostics = if guest.is_some() {
            runtime.operation(request).await?.diagnostics.clone()
        } else {
            let request = schematic_app::operations::native::ValidateSchematic { schematic: request.schematic.into() };
            schematic_app::services::sidecar::call(&runtime, request).await?.diagnostics
        };
        println!("Validation: {} diagnostics", diagnostics.len());

        // Propose a Route
        let from = snapshot.symbol(SymbolId(1)).position;
        let to = snapshot.symbol(SymbolId(2)).position;
        let request = ProposeRoute {
            from: model::Point { x: from.x, y: from.y },
            to: model::Point { x: to.x, y: to.y },
        };
        let proposed = if guest.is_some() {
            runtime.operation(request).await?.points.clone()
        } else {
            let request = schematic_app::operations::native::ProposeRoute {
                from: schematic_app::operations::native::Point { x: request.from.x, y: request.from.y },
                to: schematic_app::operations::native::Point { x: request.to.x, y: request.to.y },
            };
            schematic_app::services::sidecar::call(&runtime, request)
                .await?
                .points
                .into_iter()
                .map(|point| model::Point { x: point.x, y: point.y })
                .collect()
        };

        // Apply the Route
        let points = proposed.iter().map(|point| Point::new(point.x, point.y)).collect();
        let request = ApplyRoute { net: NetId(1), points };
        state.commit(request).await?;
        println!("Applied route: {:?}", proposed);

        // Undo the Route
        let selection = Focus::all();
        state.undo(principal.clone(), selection).await?;

        // Redo the Route
        let selection = Focus::all();
        state.redo(principal, selection).await?;
        println!("Undid and redid the route");

        if advanced {
            crate::advanced::run(&runtime, schematic, guest.is_none()).await?;
        }
        Ok::<_, crate::Error>(())
    }
    .await;

    // Settle the Plugin and Runtime
    let stopped = plugin.stop().await;
    let snapshot = runtime.state.state().read(Clone::clone).await;
    let settled = runtime.shutdown().await;
    result?;
    stopped?;
    settled?;

    // Save and Reopen the Settled Document
    let snapshot = snapshot?;
    Document::save(&path, snapshot, &checkpoint)?;
    let document = Document::open(&path)?;
    let reopened = app::launch(document, &config).await?;
    let saved = reopened.state.state().read(Clone::clone).await;
    reopened.shutdown().await?;
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

/* === Tests === */
