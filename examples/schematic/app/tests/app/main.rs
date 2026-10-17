/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

// Third Party Imports

// Local Imports
use harmos::{
    Application, DefinitionId, Error, runtime::Catalog, runtime::Registration, state::At,
    state::Focus, state::Kind, state::Position, state::PrincipalId, state::Receipt,
    state::StateManager, streams::Item,
};
use schematic_app::app::{self, Runtime, SchematicEditor};
use schematic_app::config::Settings;
use schematic_app::document::Document;
use schematic_app::error::LaunchError;
use schematic_app::records::{ExportCompleted, ExportFormat, Records};
use schematic_app::state::{NetId, NetName, PinRef, Point, Rotation, Schematic, SymbolId};
use schematic_app::streams::NetVoltage;
use schematic_app::transactions::Transactions;
use schematic_app::transactions::net::{ApplyRoute, ConnectPins, RenameNet};
use schematic_app::transactions::symbol::{MoveSymbol, PlaceSymbol};
use schematic_app::{analysis, error, storage};

/* === Constants === */
const R1: SymbolId = SymbolId(1);
const U3: SymbolId = SymbolId(2);

const N1: NetId = NetId(1);
const N2: NetId = NetId(2);

const VERSION_1_RENAME: &str = concat!(
    r#"{"position":POSITION,"kind":"Transaction","id":"net.rename","version":1,"#,
    r#""metadata":{"principal":"alice","correlation":POSITION,"causation":null,"#,
    r#""request":null,"link":null,"#,
    r#""at_us":0},"#,
    r#""payload":PAYLOAD}"#,
);

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

/* === Private Functions === */

#[tokio::test]
async fn route_history_reopens() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schematic.json");
    let document = Document::new(&path);
    let configuration = config(&path);
    let recovery = document.recovering().unwrap();
    let checkpoint = recovery.sidecars();
    let origin = document.into_origin();
    let assembly = app::builder(origin, ());
    let assembly = storage::attach(assembly, recovery, &configuration).unwrap();
    let editor = assembly.runtime().await.unwrap();
    let state = editor.state.as_principal(alice());
    edit(&state).await;

    let points = vec![Point::new(10, 45), Point::new(60, 45), Point::new(60, 40)];
    let proposal = ApplyRoute {
        net: N1,
        points: points.clone(),
    };
    state.commit(proposal).await.unwrap();
    assert_eq!(whole(&editor).await.net(N1).route, points);
    state.undo(alice(), Focus::all()).await.unwrap();
    assert!(whole(&editor).await.net(N1).route.is_empty());
    state.redo(alice(), Focus::all()).await.unwrap();
    assert_eq!(whole(&editor).await.net(N1).route, points);

    let invalid = ApplyRoute {
        net: N1,
        points: vec![Point::new(0, 0), Point::new(1, 1)],
    };
    assert!(state.commit(invalid).await.is_err());
    assert_eq!(whole(&editor).await.net(N1).route, points);

    let snapshot = editor.state.state().read(Clone::clone).await.unwrap();
    editor.stop().await.unwrap();
    Document::save(&path, snapshot, &checkpoint).unwrap();
    let document = Document::open(&path).unwrap();
    let reopened = app::launch(document, &configuration).await.unwrap();
    assert_eq!(whole(&reopened).await.net(N1).route, points);
    reopened.stop().await.unwrap();
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("editor_edits_undoes", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let editor = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");
                    let handle = editor.state.as_principal(alice());

                    let edits = edit(&handle).await;
                    let renamed = handle
                        .commit(RenameNet {
                            net: N1,
                            to: NetName::parse("VCC"),
                        })
                        .await
                        .expect("no other net answers to VCC");

                    assert_eq!(edits.moved, Position(4));
                    assert_eq!(renamed.position, Position(5));
                    assert_eq!(whole(&editor).await.symbol(R1).position, Point::new(10, 45));

                    // The scope narrows to one symbol's own edits, so the walk passes over the
                    // rename above it and takes the move — which is the whole point of a
                    // grouping key living in the payload the application declared.
                    let undone = handle
                        .undo(alice(), only(R1))
                        .await
                        .expect("the move carries its own inverse");
                    let after = whole(&editor).await;

                    assert_eq!(undone.position, Position(6));
                    assert_eq!(after.symbol(R1).position, Point::new(10, 20));
                    assert_eq!(
                        after.net(N1).name,
                        NetName::parse("VCC"),
                        "the rename was outside the scope and was never a candidate"
                    );

                    let redone = handle
                        .redo(alice(), only(R1))
                        .await
                        .expect("the move alice just undid is hers to put back");

                    assert_eq!(redone.position, Position(7));
                    assert_eq!(whole(&editor).await.symbol(R1).position, Point::new(10, 45));

                    // Undo is personal: bob has committed nothing, so there is nothing of his
                    // to take back however long alice's history is.
                    assert!(matches!(
                        handle.undo(PrincipalId::new("bob"), Focus::all()).await,
                        Err(Error::NothingToUndo)
                    ));

                    drop(handle);
                    editor.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("s_own_vocabulary", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let editor = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");

                    let refused = editor
                        .state
                        .commit(MoveSymbol {
                            symbol: SymbolId(7),
                            to: Point::new(10, 20),
                        })
                        .await;

                    assert!(
                        matches!(
                            refused,
                            Err(Error::Rejected(error::SchematicError::UnknownSymbol(
                                SymbolId(7)
                            )))
                        ),
                        "a rejection is a domain answer, not a framework code"
                    );
                    assert_eq!(
                        editor.state.applied(),
                        Position::ORIGIN,
                        "a refused change mutates nothing and appends nothing"
                    );
                    editor.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("sidecar_restores_state", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let document = scratch("sidecar-reboot").join("schematic.json");

                    // The live path: the same four edits in memory, then one undo, so there is
                    // something for the recovered runtime's own fold to be equal to.
                    let live = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");
                    let handle = live.state.as_principal(alice());
                    edit(&handle).await;
                    let before_undo = whole(&live).await;
                    handle
                        .undo(alice(), Focus::all())
                        .await
                        .expect("the move is the latest still-active entry");
                    let after_undo = whole(&live).await;
                    drop(handle);
                    live.stop().await.unwrap();

                    // The persisted path: the same four edits, and then the process dies with
                    // nothing saved.
                    let editor = opened(&document).await;
                    let handle = editor.state.as_principal(alice());
                    let edits = edit(&handle).await;
                    drop(handle);
                    let settled = editor.stop().await.unwrap();

                    assert_eq!(settled, edits.moved, "every edit reached the sidecar");

                    let recovered = opened(&document).await;

                    assert_eq!(recovered.state.applied(), Position(4));
                    assert_eq!(
                        whole(&recovered).await,
                        before_undo,
                        "the artifact was the origin and the sidecar supplied the entries since"
                    );

                    // The undo stack is a fold over links the recovery replayed, so it comes
                    // back without a single inverse ever having been stored.
                    let handle = recovered.state.as_principal(alice());
                    let undone = handle
                        .undo(alice(), Focus::all())
                        .await
                        .expect("the rebuilt stack knows the move as well as the live one did");

                    assert_eq!(undone.position, Position(5));
                    assert_eq!(
                        whole(&recovered).await,
                        after_undo,
                        "the inverse was re-derived against the state before its own entry"
                    );

                    drop(handle);
                    recovered.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("rename_decodes_applies", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let document = scratch("version-1-rename").join("schematic.json");
    let sidecar = Document::new(&document).journal().to_owned();

    let editor = opened(&document).await;
    let handle = editor.state.as_principal(alice());
    edit(&handle).await;
    let connected = handle
        .commit(ConnectPins {
            net: N2,
            name: NetName::verbatim("GND"),
            pins: BTreeSet::from([PinRef::new(R1, 2), PinRef::new(U3, 8)]),
        })
        .await
        .expect("both symbols are placed");
    drop(handle);
    editor.stop().await.unwrap();

    // Two entries a previous build wrote, in the shape it wrote them: the new
    // name as raw text, stamped version 1. The second renames a net to a name
    // the first has already taken — which version 1 allowed and version 2 does
    // not.
    append(
        &sidecar,
        connected.frontier.0 + 1,
        r#"{"net":1,"to":"DATA[3]"}"#,
    );
    append(
        &sidecar,
        connected.frontier.0 + 2,
        r#"{"net":2,"to":"DATA[3]"}"#,
    );

    let recovered = opened(&document).await;
    let schematic = whole(&recovered).await;

    assert_eq!(
        recovered.state.applied(),
        Position(connected.frontier.0 + 2)
    );
    assert_eq!(
        schematic.net(N1).name,
        NetName::verbatim("DATA[3]"),
        "version 1 applied the text verbatim, so carrying it forward keeps that \
         reading rather than parsing it under today's rules"
    );
    assert_eq!(schematic.net(N1).name.member, None);
    assert_eq!(schematic.net(N2).name, NetName::verbatim("DATA[3]"));

    // History is settled: today's check would refuse exactly what those two
    // entries did, and replay never consulted it.
    let handle = recovered.state.as_principal(alice());
    let refused = handle
        .commit(RenameNet {
            net: N1,
            to: NetName::verbatim("DATA[3]"),
        })
        .await;

    assert!(
        matches!(refused, Err(Error::Rejected(error::SchematicError::NameTaken(name))) if name == "DATA[3]")
    );

    // The same text, declared today, is a different name — which is exactly
    // what the version stamp beside a stored entry is for.
    handle
        .commit(RenameNet {
            net: N1,
            to: NetName::parse("DATA[3]"),
        })
        .await
        .expect("the parsed name is not the verbatim one, so it is free");

    assert_eq!(whole(&recovered).await.net(N1).name.member, Some(3));

    drop(handle);
    recovered.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("definition_register_once", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let registered: BTreeMap<(String, u32), Kind> =
        definitions_of::<SchematicEditor>(Transactions::catalog())
            .into_iter()
            .chain(definitions_of::<SchematicEditor>(Records::catalog()))
            .map(|(kind, id, version)| ((id.as_str().to_owned(), version), kind))
            .collect();

    assert_eq!(
        registered,
        BTreeMap::from([
            (("export.completed".to_owned(), 1), Kind::Record),
            (("net.connect".to_owned(), 1), Kind::Transaction),
            (("net.rename".to_owned(), 1), Kind::Transaction),
            (("net.rename".to_owned(), 2), Kind::Transaction),
            (("net.route".to_owned(), 1), Kind::Transaction),
            (("symbol.move".to_owned(), 1), Kind::Transaction),
            (("symbol.place".to_owned(), 1), Kind::Transaction),
        ]),
        "one variant per definition, plus every earlier version its history declares"
    );

});
        });
    }
    {
        scenario_runtime::check("fact_joins_ordered", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let editor = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");
                    let handle = editor.state.as_principal(alice());

                    edit(&handle).await;
                    let exported = harmos::records::RecordManager::new(handle.clone())
                        .publish(ExportCompleted {
                            format: ExportFormat::Svg,
                            symbols: 2,
                        })
                        .await
                        .expect("a fact cannot be refused");
                    editor.state.wait_until_stored(exported.position).await;

                    let facts: Vec<Records> = editor
                        .state
                        .query::<Records>(Position::ORIGIN)
                        .await
                        .map(|entry| entry.expect("the tail reaches back to the origin").value)
                        .collect();

                    assert_eq!(exported.position, Position(5));
                    assert_eq!(
                        facts,
                        vec![Records::ExportCompleted(ExportCompleted {
                            format: ExportFormat::Svg,
                            symbols: 2,
                        })]
                    );
                    assert_eq!(
                        editor.state.stored(),
                        editor.state.applied(),
                        "with nothing attached, stored tracks applied on the same code path"
                    );

                    drop(handle);
                    editor.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("editor_feed_runs", || {
            let mut builder = tokio::runtime::Builder::new_multi_thread();
            builder.worker_threads(2);
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let editor = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");
                    let committed = editor
                        .state
                        .commit(PlaceSymbol {
                            symbol: R1,
                            reference: "R1".to_owned(),
                            at: Point::new(10, 20),
                            rotation: Rotation::Zero,
                        })
                        .await
                        .expect("R1 is free");
                    let demand = editor.streams.demand::<NetVoltage>();
                    let mut watching = editor.streams.watch::<NetVoltage>();

                    assert_eq!(*demand.borrow(), 1, "the watcher starts the capture demand");
                    editor.streams.publish(NetVoltage::teaching(1_000, 3_300));
                    editor.streams.publish(NetVoltage::teaching(1_750, 3_290));

                    let cut: Vec<At<NetVoltage>> = editor
                        .streams
                        .query::<NetVoltage>(..)
                        .as_of(committed.position)
                        .await
                        .expect("nothing was evicted yet")
                        .collect();
                    assert_eq!(cut.len(), 2);
                    assert!(
                        cut.iter().all(|row| row.position == committed.position),
                        "each row witnessed the committed document state"
                    );

                    for (sampled_at_us, millivolts) in [
                        (2_900, 3_310),
                        (4_400, 3_305),
                        (7_100, 3_295),
                        (9_800, 3_300),
                    ] {
                        editor
                            .streams
                            .publish(NetVoltage::teaching(sampled_at_us, millivolts));
                    }
                    assert_eq!(
                        next_stream(&mut watching).await,
                        Some(Item::Gap { missed: 2 })
                    );
                    for expected in [2_900, 4_400, 7_100, 9_800] {
                        assert!(matches!(
                            next_stream(&mut watching).await,
                            Some(Item::Row(At { value, position }))
                                if value.sampled_at_us == expected && position == committed.position
                        ));
                    }

                    drop(watching);
                    assert_eq!(*demand.borrow(), 0, "dropping the watcher stops demand");
                    editor.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("storage_ac002_recorded", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("stream-readback");
                    let editor = recording(Schematic::default(), &directory)
                        .await
                        .expect("the selected recording boots");
                    let committed = editor
                        .state
                        .commit(PlaceSymbol {
                            symbol: R1,
                            reference: "R1".to_owned(),
                            at: Point::new(10, 20),
                            rotation: Rotation::Zero,
                        })
                        .await
                        .expect("R1 is free");

                    for sample in 0..6 {
                        editor.streams.publish(NetVoltage::teaching(sample, 3_300));
                    }
                    editor.stop().await.unwrap();

                    assert_eq!(
                        analysis::read(&directory),
                        analysis::Columns {
                            position: vec![committed.position.0; 4],
                            gap_missed: vec![Some(2), None, None, None],
                        },
                        "the footered file reads with its journal join and exact loss evidence"
                    );
                });
        });
    }
    {
        scenario_runtime::check("saved_document_reopens", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = scratch("document-open").join("schematic.json");
                    let editor = memory_only(Schematic::default())
                        .await
                        .expect("a memory-only boot recovers nothing");
                    let handle = editor.state.as_principal(alice());
                    let edits = edit(&handle).await;
                    let saved = whole(&editor).await;
                    drop(handle);
                    editor.stop().await.unwrap();

                    // The artifact a save writes: the state, the generation it was written at,
                    // and the position that state already folds. Hand-encoded, because what is
                    // under test is the reader.
                    std::fs::write(
                        &path,
                        serde_json::to_vec(&serde_json::json!({
                            "generation": 2,
                            "position": edits.moved,
                            "schematic": saved,
                        }))
                        .expect("the schematic serializes"),
                    )
                    .expect("the scratch directory is writable");

                    let document = Document::open(&path).expect("the saved document reads back");

                    assert_eq!(document.generation(), 2);
                    assert_eq!(document.journal(), path.with_extension("jsonl"));

                    let reopened = app::launch(document, &config(&path))
                        .await
                        .expect("a fresh sidecar stands beside the second generation");

                    assert_eq!(
                        reopened.state.applied(),
                        edits.moved,
                        "the artifact is the origin, so history resumes where it was saved"
                    );
                    assert_eq!(whole(&reopened).await, saved);
                    reopened.stop().await.unwrap();
                });
        });
    }
}

fn definitions_of<A: Application>(catalog: impl Registration<A>) -> Vec<(Kind, DefinitionId, u32)> {
    match catalog.register() {
        Catalog::Transactions { definitions, .. } => definitions,
        Catalog::Streams(_) | Catalog::Lanes(_) => {
            unreachable!("a transaction or fact catalog always registers as `Transactions`")
        }
    }
}

async fn next_stream<S: Clone + 'static>(
    watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin),
) -> Option<Item<S>> {
    use std::pin::Pin;

    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

async fn edit(handle: &StateManager<SchematicEditor>) -> Edits {
    place(handle, R1, "R1", Point::new(10, 20), Rotation::Zero).await;
    place(handle, U3, "U3", Point::new(60, 20), Rotation::quarters(1)).await;
    handle
        .commit(ConnectPins {
            net: N1,
            name: NetName::verbatim("N$1"),
            pins: BTreeSet::from([PinRef::new(R1, 1), PinRef::new(U3, 7)]),
        })
        .await
        .expect("both symbols are placed");
    let moved = handle
        .commit(MoveSymbol {
            symbol: R1,
            to: Point::new(10, 45),
        })
        .await
        .expect("R1 is placed");

    Edits {
        moved: moved.position,
    }
}

async fn place(
    handle: &StateManager<SchematicEditor>,
    symbol: SymbolId,
    reference: &str,
    at: Point,
    rotation: Rotation,
) -> Receipt {
    handle
        .commit(PlaceSymbol {
            symbol,
            reference: reference.to_owned(),
            at,
            rotation,
        })
        .await
        .expect("nothing is placed under that name yet")
}

fn only(symbol: SymbolId) -> Focus<SchematicEditor> {
    Focus::matching(move |change: &Transactions| change.symbol() == Some(symbol))
}

async fn opened(document: &Path) -> Runtime {
    app::launch(Document::new(document), &config(document))
        .await
        .expect("the sidecar belongs to this artifact and replays")
}

fn config(document: &Path) -> Settings {
    Settings::new(document.with_file_name("recordings"))
}

async fn memory_only(origin: Schematic) -> Result<Runtime, LaunchError> {
    Ok(app::builder(origin, ()).runtime().await?)
}

async fn recording(origin: Schematic, directory: &Path) -> Result<Runtime, LaunchError> {
    let assembly = app::builder(origin, ());

    Ok(storage::recordings(assembly, &Settings::new(directory))?
        .runtime()
        .await?)
}

async fn whole(editor: &Runtime) -> Schematic {
    editor
        .state
        .state()
        .read(Clone::clone)
        .await
        .expect("reads are open")
        .value
}

fn append(sidecar: &Path, position: u64, payload: &str) {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(sidecar)
        .expect("the sidecar is there");
    let line = VERSION_1_RENAME
        .replace("POSITION", &position.to_string())
        .replace("PAYLOAD", payload);

    writeln!(file, "{line}").expect("the sidecar is writable");
}

fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);

    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the target directory is writable");

    directory
}

/* === Tests === */
