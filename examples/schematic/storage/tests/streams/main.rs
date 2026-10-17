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

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::io::Write;
// Third Party Imports

// Local Imports
use harmos::storage::StreamConsumer;
use schematic_storage::streams::{JsonlStreams, read_rows};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Model;

#[harmos::stream(id = "reading", window = 8)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Reading {
    value: u64,
}

#[derive(serde::Deserialize, PartialEq, Eq, Debug)]
struct Columns {
    position: u64,
    gap_missed: Option<u64>,
}

/* === Enums === */
#[harmos::streams]
enum Feeds {
    Reading(Reading),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Model {
    type State = support::Ledger;
    type Transaction = support::Change;
    type Record = support::Fact;
    type Streams = Feeds;
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("missing_checkpoint_origin", || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let temporary = tempfile::tempdir().unwrap();
                    let directory = temporary.path();
                    let capture = JsonlStreams::<Model>::open(directory).unwrap();
                    assert_eq!(
                        capture.resume(),
                        harmos::storage::Cursor::ORIGIN,
                        "a directory nothing has recorded into resumes from the origin"
                    );
                    let runtime = harmos::Runtime::<Model>::builder(support::Ledger::default(), ())
                        .register(Feeds::catalog())
                        .attach_streams(capture.select::<Reading>())
                        .runtime()
                        .await
                        .unwrap();
                    for value in 1..=7 {
                        runtime.streams.publish(Reading { value });
                    }
                    runtime.stop().await.unwrap();
                    assert_eq!(
                        JsonlStreams::<Model>::open(directory).unwrap().resume(),
                        harmos::storage::Cursor(7),
                        "a cursor survives the process that wrote it"
                    );
                    assert_eq!(
                        JsonlStreams::<Model>::open(directory).unwrap().resume(),
                        harmos::storage::Cursor(7),
                        "healing repairs captures and never touches the checkpoint beside them"
                    );
                });
        });
    }
    {
        scenario_runtime::check("torn_final_line", || {
            let temporary = tempfile::tempdir().unwrap();
            let directory = temporary.path();
            std::fs::create_dir_all(directory).unwrap();
            let torn = directory.join("torn.jsonl");
            let whole = directory.join("whole.jsonl");
            std::fs::write(&torn, "{\"position\":1}\n{\"positi").unwrap();
            std::fs::write(&whole, "{\"position\":1}\n").unwrap();

            JsonlStreams::<Model>::open(directory).unwrap();

            assert_eq!(
                std::fs::read_to_string(&torn).unwrap(),
                "{\"position\":1}\n",
                "the line an interrupted write never finished is gone"
            );
            assert_eq!(
                std::fs::read_to_string(&whole).unwrap(),
                "{\"position\":1}\n",
                "and a capture that ended on a newline is untouched"
            );
        });
    }
    {
        scenario_runtime::check("file_complete_line", || {
            let temporary = tempfile::tempdir().unwrap();
            let directory = temporary.path();
            std::fs::create_dir_all(directory).unwrap();
            let path = directory.join("only.jsonl");
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(vec![b'x'; 64 * 1024 + 7].as_slice())
                .unwrap();
            drop(file);

            JsonlStreams::<Model>::open(directory).unwrap();

            assert_eq!(
                std::fs::metadata(&path).unwrap().len(),
                0,
                "nothing in it was ever a readable row"
            );
        });
    }
    {
        scenario_runtime::check("technical_members_read", || {
            let temporary = tempfile::tempdir().unwrap();
            let directory = temporary.path();
            std::fs::create_dir_all(directory).unwrap();
            let path = directory.join("rows.jsonl");
            std::fs::write(
                &path,
                "{\"volts\":1.5,\"position\":7,\"gap_missed\":null}\n\
         {\"volts\":2.5,\"position\":9,\"gap_missed\":3}\n",
            )
            .unwrap();

            assert_eq!(
                read_rows::<Columns>(&path).unwrap(),
                vec![
                    Columns {
                        position: 7,
                        gap_missed: None
                    },
                    Columns {
                        position: 9,
                        gap_missed: Some(3)
                    },
                ],
                "a struct naming only the technical members reads the whole capture"
            );
        });
    }
}

/* === Tests === */
