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
use std::pin::Pin;
// Third Party Imports

// Local Imports
use crate::error::Error;
use harmos::{At, Focus, Item, Position, PrincipalId, StateManager};
use minischematic_app::app::{self, Runtime, SchematicEditor};
use minischematic_app::config::Config;
use minischematic_app::document::Document;
use minischematic_app::error::LaunchError;
use minischematic_app::records::{ExportCompleted, ExportFormat, Records};
use minischematic_app::state::{NetId, NetName, PinRef, Point, Rotation, Schematic, SymbolId};
use minischematic_app::streams::NetVoltage;
use minischematic_app::transactions::Transactions;
use minischematic_app::transactions::net::{ConnectPins, RenameNet};
use minischematic_app::transactions::symbol::{MoveSymbol, PlaceSymbol};
use minischematic_app::{analysis, storage};

/* === Constants === */
const R1: SymbolId = SymbolId(1);
const U3: SymbolId = SymbolId(2);

const N1: NetId = NetId(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Edits {
    moved: Position,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(document: Document, config: &Config) -> Result<(), Error> {
    println!("a schematic editor on harmos\n");

    let live = in_memory(config).await?;
    persisted(live, document, config).await?;

    Ok(())
}

/* === Private Functions === */
async fn recording(origin: Schematic, config: &Config) -> Result<Runtime, LaunchError> {
    Ok(storage::recordings(app::builder(origin, ()), config)?
        .runtime()
        .await?)
}

async fn in_memory(config: &Config) -> Result<Schematic, Error> {
    println!("1  memory-only boot");
    let runtime = recording(Schematic::default(), config).await?;
    println!(
        "   the runtime stands on an empty schematic at position {}\n",
        runtime.state.applied()
    );

    println!("2  editing, as alice");
    let handle = runtime.state.as_principal(alice());
    let edits = edit(&handle).await;

    println!("3  one feed, alive while somebody needs it live or recorded");
    let capture = stream(&runtime, edits.moved).await;

    let rename = handle
        .commit(RenameNet {
            net: N1,
            to: NetName::parse("VCC"),
        })
        .await
        .expect("no other net is called VCC");
    println!("   [{}] renamed net 1 to VCC", rename.position);
    println!("   {}\n", read(&runtime).await);

    println!("4  undo, narrowed to one symbol");
    let undone = handle
        .undo(alice(), only(R1))
        .await
        .expect("alice moved R1 and the move carries its own inverse");
    println!(
        "   undo(alice, edits to R1) -> [{}], which undoes [{}]",
        undone.position, edits.moved
    );
    println!("   {}", read(&runtime).await);
    println!("   the rename was outside the scope, so it stands untouched");
    let redone = handle
        .redo(alice(), only(R1))
        .await
        .expect("the move alice just undid is hers to put back");
    println!(
        "   redo(alice, edits to R1) -> [{}], which redoes [{}]",
        redone.position, edits.moved
    );
    println!("   {}\n", read(&runtime).await);

    println!("5  a fact, and the gate that lets it leave the process");
    let exported = harmos::RecordManager::new(handle.clone())
        .publish(ExportCompleted {
            format: ExportFormat::Svg,
            symbols: 2,
        })
        .await
        .expect("a fact cannot be refused");
    runtime.state.wait_until_stored(exported.position).await;
    println!(
        "   [{}] ExportCompleted {{ format: {}, symbols: 2 }}",
        exported.position,
        ExportFormat::Svg
    );
    println!(
        "   applied {} · stored {} — the export may be announced",
        runtime.state.applied(),
        runtime.state.stored()
    );
    println!("   query::<Records>(0) -> {:?}\n", facts(&runtime).await);

    let schematic = whole(&runtime).await;
    drop(handle);
    runtime.stop().await?;
    capture.await.expect("the capture task stops without panic");
    let columns = analysis::read(config.recordings());
    println!("   demand 0 -> capture stopped; the recording is durable");
    println!("   recorded position   -> {:?}", columns.position);
    println!("   recorded gap_missed -> {:?}\n", columns.gap_missed);

    Ok(schematic)
}

async fn persisted(live: Schematic, document: Document, config: &Config) -> Result<(), Error> {
    println!("6  persistence, through a sidecar journal beside the document");
    println!("   sidecar {}", document.sidecar().display());

    // A fresh document: the artifact is the empty schematic saved as
    // generation 1, and the sidecar carries everything committed since.
    let runtime = app::launch(document.clone(), config).await?;
    let handle = runtime.state.as_principal(alice());
    let edits = edit(&handle).await;
    let lost = whole(&runtime).await;
    drop(handle);

    let settled = runtime.stop().await?;
    println!("   the process ends at {settled} without a save\n");

    println!("7  reboot from the sidecar");
    let runtime = app::launch(document, config).await?;
    println!("   {}", read(&runtime).await);
    println!(
        "   the recovered schematic equals the one that was lost: {}",
        lost == whole(&runtime).await
    );
    println!(
        "   (and it is not the memory-only session's, which had one more edit: {})",
        live != whole(&runtime).await
    );

    // The undo stack is a fold over links the recovery just replayed, so it
    // comes back without a single inverse having been written down.
    let handle = runtime.state.as_principal(alice());
    let undone = handle
        .undo(alice(), Focus::all())
        .await
        .expect("the rebuilt stack knows the move as well as the live one did");
    println!(
        "   undo(alice, everything) -> [{}], which undoes [{}]",
        undone.position, edits.moved
    );
    println!("   {}", read(&runtime).await);
    drop(handle);
    runtime.stop().await?;

    Ok(())
}

async fn stream(runtime: &Runtime, committed: Position) -> tokio::task::JoinHandle<()> {
    let streams = runtime.streams.clone();
    let mut watching = runtime.streams.watch::<NetVoltage>();
    let mut demand = streams.demand::<NetVoltage>();
    let mut observed_demand = demand.clone();
    let (reported, report) = tokio::sync::oneshot::channel();

    // Application code owns the source. Harmos reports demand; this task
    // decides that demand starts the simulator and its return to zero stops it.
    // The recording and watcher each hold one place until their own lifetime
    // ends.
    let capture = tokio::spawn(async move {
        assert_eq!(*demand.borrow(), 2);
        streams.publish(NetVoltage::teaching(1_000, 3_300));
        streams.publish(NetVoltage::teaching(1_750, 3_290));

        let cut = streams
            .query::<NetVoltage>(..)
            .as_of(committed)
            .await
            .expect("the first two rows are still retained")
            .count();

        for (sampled_at_us, millivolts) in [
            (2_900, 3_310),
            (4_400, 3_305),
            (7_100, 3_295),
            (9_800, 3_300),
        ] {
            streams.publish(NetVoltage::teaching(sampled_at_us, millivolts));
        }
        reported.send(cut).ok();

        while demand.changed().await.is_ok() {
            if *demand.borrow() == 0 {
                break;
            }
        }
    });

    let cut = report
        .await
        .expect("capture reports its state-consistent cut");
    println!("   demand 2 -> recording + watcher started capture");
    println!(
        "   as_of({committed}) -> {} rows stamped at the committed move",
        cut
    );

    assert_eq!(
        next(&mut watching).await,
        Some(Item::Gap { missed: 2 }),
        "six publishes into a four-row window lose exactly two rows"
    );
    println!("   watch -> Gap {{ missed: 2 }}, then the four retained rows");
    for _ in 0..4 {
        let Some(Item::Row(At { value, position })) = next(&mut watching).await else {
            panic!("the watcher continues with a retained voltage row");
        };
        println!(
            "   row @{} · {} µs · {} mV",
            position, value.sampled_at_us, value.millivolts
        );
    }

    drop(watching);
    while *observed_demand.borrow() != 1 {
        observed_demand
            .changed()
            .await
            .expect("the recording still holds demand");
    }
    println!("   demand 1 -> recording keeps capture requested\n");

    capture
}

async fn next<S: Clone + 'static>(
    watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin),
) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

async fn edit(handle: &StateManager<SchematicEditor>) -> Edits {
    let placed = handle
        .commit(PlaceSymbol {
            symbol: R1,
            reference: "R1".to_owned(),
            at: Point::new(10, 20),
            rotation: Rotation::Zero,
        })
        .await
        .expect("nothing is placed under that name yet");
    println!("   [{}] placed R1 at (10, 20)", placed.position);

    let placed = handle
        .commit(PlaceSymbol {
            symbol: U3,
            reference: "U3".to_owned(),
            at: Point::new(60, 20),
            rotation: Rotation::quarters(1),
        })
        .await
        .expect("nothing is placed under that name yet");
    println!(
        "   [{}] placed U3 at (60, 20), turned a quarter",
        placed.position
    );

    let connected = handle
        .commit(ConnectPins {
            net: N1,
            name: NetName::verbatim("N$1"),
            pins: BTreeSet::from([PinRef::new(R1, 1), PinRef::new(U3, 7)]),
        })
        .await
        .expect("both symbols are placed");
    println!(
        "   [{}] connected {} and {} on net N$1",
        connected.position,
        PinRef::new(R1, 1),
        PinRef::new(U3, 7)
    );

    let moved = handle
        .commit(MoveSymbol {
            symbol: R1,
            to: Point::new(10, 45),
        })
        .await
        .expect("R1 is placed");
    println!("   [{}] moved R1 to (10, 45)", moved.position);

    Edits {
        moved: moved.position,
    }
}

fn only(symbol: SymbolId) -> Focus<SchematicEditor> {
    Focus::matching(move |change: &Transactions| change.symbol() == Some(symbol))
}

async fn read(runtime: &Runtime) -> String {
    let read = runtime
        .state
        .state()
        .read(|schematic| {
            let symbols = schematic.symbols.len();
            let nets = schematic.nets.len();
            let position = schematic.symbol(R1).position;
            let name = schematic.net(N1).name.to_string();

            format!("{symbols} symbols, {nets} net · R1 at {position} · net 1 is {name}")
        })
        .await
        .expect("reads are open");

    format!("read @{} · {}", read.position, read.value)
}

async fn whole(runtime: &Runtime) -> Schematic {
    runtime
        .state
        .state()
        .read(Clone::clone)
        .await
        .expect("reads are open")
        .value
}

async fn facts(runtime: &Runtime) -> Vec<Records> {
    runtime
        .state
        .query::<Records>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the tail reaches back to the origin").value)
        .collect()
}

fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

/* === Tests === */
