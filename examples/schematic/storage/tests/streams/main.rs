/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "../support/mod.rs"]
mod support;

use std::io::Write;

use harmos::storage::StreamConsumer;
use schematic_storage::streams::{JsonlStreams, read_rows};

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

#[harmos::lanes]
pub enum TestLanes {
    #[default]
    Open,
}

#[harmos::streams]
enum Feeds {
    Reading(Reading),
}

impl harmos::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos::Startup<Self>;

    async fn prepare(_: (), _setup: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos::Application for Model {
    type State = support::Ledger;
    type Transaction = support::Change;
    type Record = support::Fact;
    type Streams = Feeds;
    type Lanes = TestLanes;
}

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let temporary = tempfile::tempdir().unwrap();
                let directory = temporary.path();
                let capture = JsonlStreams::<Model>::open(directory).unwrap();
                assert_eq!(
                    capture.resume(),
                    harmos::storage::Cursor::ORIGIN,
                    "a directory nothing has recorded into resumes from the origin"
                );
                let runtime = harmos::Runtime::<Model>::launch_with(support::Ledger::default(), (), |setup| {
                    setup.settings().register(Feeds::catalog()).attach_streams(capture.select::<Reading>());
                })
                .await
                .unwrap();
                for value in 1..=7 {
                    runtime.streams.publish(Reading { value });
                }
                runtime.shutdown().await.unwrap();
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
        }
    }
    {
        {
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
        }
    }
    {
        {
            let temporary = tempfile::tempdir().unwrap();
            let directory = temporary.path();
            std::fs::create_dir_all(directory).unwrap();
            let path = directory.join("only.jsonl");
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(vec![b'x'; 64 * 1024 + 7].as_slice()).unwrap();
            drop(file);

            JsonlStreams::<Model>::open(directory).unwrap();

            assert_eq!(std::fs::metadata(&path).unwrap().len(), 0, "nothing in it was ever a readable row");
        }
    }
    {
        {
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
                vec![Columns { position: 7, gap_missed: None }, Columns { position: 9, gap_missed: Some(3) },],
                "a struct naming only the technical members reads the whole capture"
            );
        }
    }
}
