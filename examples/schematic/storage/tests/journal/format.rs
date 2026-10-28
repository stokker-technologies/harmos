/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use harmos::{
    state::CausationId, state::CorrelationId, state::Entry, state::FactSource, state::Focus, state::Link,
    state::Metadata as JournalMetadata, state::Position, state::PrincipalId, state::Recorded, state::RequestKey, state::SystemFact,
    state::Timestamp, storage::JournalConsumer,
};
use schematic_storage::Jsonl;
use support::{Bank, Ledger, Noted, alice, booted, deposit, ledger, scratch, withdraw};

const GOLDEN: &str = include_str!("../fixtures/journal.v1.jsonl");

const NON_CANONICAL: &str = r#"{ "kind": "Transaction", "version": 1, "position": 4, "id": "ledger.bonus", "payload": { "amount": 3 }, "metadata": { "at_us": -1000001, "correlation": 4, "principal": null, "causation": null, "request": null, "link": null } }"#;

const SYSTEM_FACTS: &str = include_str!("../fixtures/system-facts.v1.jsonl");

struct Witnesses {
    seen: Arc<Mutex<Vec<(Position, JournalMetadata)>>>,
    resume: Position,
}

impl JournalConsumer<Bank> for Witnesses {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<Bank>>>>, applied: Position) -> Position {
        let mut seen = self.seen.lock().expect("the probe is never poisoned");

        for entry in entries {
            self.resume = entry.position;
            seen.push((entry.position, entry.metadata.clone()));
        }

        applied.min(self.resume)
    }
}

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let copy = scratch("frozen-wire").join("journal.jsonl");
    let before = micros();

    let live = booted(Ledger::default(), |setup| {
        setup.settings().recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"));

    })
    .await;
    let handle = live.state.as_principal(alice());
    let deposited = handle.commit(deposit(7)).await.expect("accepted");
    handle
        .caused_by(deposited.position)
        .commit_keyed(RequestKey::new("pay-once"), withdraw(2))
        .await
        .expect("accepted");
    live.records
        .publish(Noted("audited".to_owned()))
        .await
        .expect("a fact cannot be refused");
    live.state
        .undo(alice(), Focus::all())
        .await
        .expect("the withdraw is invertible");
    drop(handle);
    live.shutdown().await.unwrap();

    let after = micros();
    let file = std::fs::read_to_string(&copy).expect("the copy is there");
    let lines: Vec<&str> = file.lines().collect();

    assert_eq!(
        lines[0], r#"{"schema":1,"generation":0,"origin":0}"#,
        "the first line is the header, and its marker names the documented format"
    );

    let entries: Vec<&str> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 || index == 3 {
                return *line;
            }
            let prefix =
                format!("{{\"group\":1,\"first\":{index},\"frontier\":{index},\"entries\":[");
            line.strip_prefix(&prefix)
                .and_then(|line| line.strip_suffix("]}"))
                .expect("transactions occupy a versioned complete singleton group")
        })
        .collect();
    let lines = entries;
    let stamps: Vec<i64> = lines[1..].iter().map(|line| at_us(line)).collect();
    for pair in stamps.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "witnessed instants follow the order they were sealed in"
        );
    }
    assert!(
        (before..=after).contains(&stamps[0]) && (before..=after).contains(&stamps[3]),
        "at_us is whole microseconds since the Unix epoch: {stamps:?} outside {before}..={after}"
    );

    assert_eq!(
        lines[1],
        format!(
            r#"{{"position":1,"kind":"Transaction","id":"ledger.deposit","version":1,"metadata":{{"principal":"alice","correlation":1,"causation":null,"request":null,"link":null,"at_us":{}}},"payload":{{"amount":7}}}}"#,
            stamps[0]
        ),
        "an entry line is the documented members in the documented order"
    );
    assert_eq!(
        lines[2],
        format!(
            r#"{{"position":2,"kind":"Transaction","id":"ledger.withdraw","version":1,"metadata":{{"principal":"alice","correlation":1,"causation":1,"request":"pay-once","link":null,"at_us":{}}},"payload":{{"amount":2}}}}"#,
            stamps[1]
        ),
        "a caused, keyed commit writes every witness member it carries"
    );
    assert_eq!(
        lines[3],
        format!(
            r#"{{"position":3,"kind":"Record","id":"ledger.noted","version":1,"metadata":{{"principal":null,"correlation":3,"causation":null,"request":null,"link":null,"at_us":{}}},"payload":"audited"}}"#,
            stamps[2]
        ),
        "a record line carries the same frame, and an absent witness member is null"
    );
    assert_eq!(
        lines[4],
        format!(
            r#"{{"position":4,"kind":"Transaction","id":"ledger.deposit","version":1,"metadata":{{"principal":"alice","correlation":4,"causation":null,"request":null,"link":{{"UndoOf":2}},"at_us":{}}},"payload":{{"amount":2}}}}"#,
            stamps[3]
        ),
        "an undo writes its inverse linked to the entry it answers"
    );
    assert_eq!(lines.len(), 5, "nothing else reaches the file");

});
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("golden-wire");
                let copy = directory.join("journal.jsonl");
                let mirror = directory.join("mirror.jsonl");
                std::fs::write(&copy, GOLDEN).expect("the copy is writable");
                let witnesses = Arc::new(Mutex::new(Vec::new()));

                let recovered = booted(Ledger::default(), |setup| {
                    setup.settings().recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"));
                    setup.settings().attach(Jsonl::<Bank>::open(&mirror).expect("a fresh mirror"));
                    setup.settings().attach(Witnesses {
                        seen: Arc::clone(&witnesses),
                        resume: Position::ORIGIN,
                    });
                })
                .await;

                assert_eq!(
                    ledger(&recovered).await,
                    Ledger {
                        balance: 3,
                        applications: 3,
                        payouts: Vec::new()
                    },
                    "every golden entry decodes and applies through the current shape"
                );
                assert_eq!(recovered.state.applied(), Position(4));
                recovered.shutdown().await.unwrap();

                let written = std::fs::read_to_string(&mirror).expect("the mirror is there");
                assert_eq!(
                    written, GOLDEN,
                    "a stored current entry redelivers byte-identical, so a lagging \
         durable copy equals the golden line for line"
                );
                assert!(GOLDEN.contains(NON_CANONICAL), "the fixture still carries the non-canonical line this test is about");
                assert!(written.contains(NON_CANONICAL), "the non-canonical line arrived verbatim, member order and spacing intact");

                let seen = witnesses.lock().expect("the probe is never poisoned");
                let (_, undo) =
                    seen.iter().find(|(position, _)| *position == Position(2)).expect("the golden's second entry was redelivered");
                assert_eq!(undo.principal, Some(PrincipalId::new("alice")));
                assert_eq!(undo.correlation, CorrelationId(Position(1)));
                assert_eq!(undo.causation, Some(CausationId(Position(1))));
                assert_eq!(undo.request, Some(RequestKey::new("pay-once")));
                assert_eq!(undo.link, Some(Link::UndoOf(Position(1))));
                assert_eq!(
                    undo.at,
                    Timestamp::UNIX_EPOCH + Duration::from_micros(1_700_000_000_000_001),
                    "at_us reads back as the exact microsecond instant the golden pins"
                );

                let (_, current) =
                    seen.iter().find(|(position, _)| *position == Position(4)).expect("the golden's non-canonical entry was redelivered");
                assert_eq!(
                    current.at,
                    Timestamp::UNIX_EPOCH - Duration::from_micros(1_000_001),
                    "a negative at_us decodes as the exact pre-epoch microsecond instant"
                );
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("system-fact-wire");
                let copy = directory.join("journal.jsonl");
                let mirror = directory.join("mirror.jsonl");
                std::fs::write(&copy, SYSTEM_FACTS).expect("the system-fact golden is writable");

                let recovered = booted(Ledger::default(), |setup| {
                    setup.settings().recover(Jsonl::<Bank>::open(&copy).expect("the golden opens"));
                    setup.settings().attach(Jsonl::<Bank>::open(&mirror).expect("a fresh mirror"));
                })
                .await;

                assert_eq!(ledger(&recovered).await, Ledger::default(), "system records replay no application behavior");
                let facts: Vec<SystemFact> = recovered
                    .state
                    .query::<SystemFact>(Position::ORIGIN)
                    .await
                    .map(|entry| entry.expect("the golden is inside the tail").value)
                    .collect();
                assert_eq!(
                    facts[0],
                    SystemFact {
                        source: FactSource::Work("autoroute".to_owned()),
                        name: "finding".to_owned(),
                        version: 1,
                        payload: vec![1, 2, 3],
                    }
                );
                assert_eq!(
                    facts[1],
                    SystemFact {
                        source: FactSource::Work("exporter".to_owned()),
                        name: "deactivated".to_owned(),
                        version: 1,
                        payload: vec![9, 8],
                    }
                );

                recovered.shutdown().await.unwrap();
                assert_eq!(
                    std::fs::read_to_string(&mirror).expect("the mirror is there"),
                    SYSTEM_FACTS,
                    "every admitted work frame forwards byte-identically"
                );
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let copy = scratch("misclaimed-wire").join("journal.jsonl");
    std::fs::write(
        &copy,
        concat!(
            "{\"schema\":1,\"generation\":0,\"origin\":0}\n",
            r#"{"position":1,"kind":"Transaction","id":"ledger.deposit","version":1,"metadata":{"principal":null,"correlation":1,"causation":null,"request":null,"link":null,"at":{"secs_since_epoch":0,"nanos_since_epoch":0}},"payload":{"amount":40}}"#,
            "\n"
        ),
    )
    .expect("the copy is writable");

    let Err(refused) = Jsonl::<Bank>::open(&copy) else {
        panic!("a witness without at_us must not decode, whatever the header claims");
    };

    assert!(
        refused.to_string().contains("unknown field `at`"),
        "the refusal is typed and names the undocumented member, never a silent misread: {refused}"
    );

});
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let copy = scratch("double-witness-wire").join("journal.jsonl");
    std::fs::write(
        &copy,
        concat!(
            "{\"schema\":1,\"generation\":0,\"origin\":0}\n",
            r#"{"position":1,"kind":"Transaction","id":"ledger.deposit","version":1,"metadata":{"principal":null,"correlation":1,"causation":null,"request":null,"link":null,"at_us":0,"at":{"secs_since_epoch":0,"nanos_since_epoch":0}},"payload":{"amount":40}}"#,
            "\n"
        ),
    )
    .expect("the copy is writable");

    let Err(refused) = Jsonl::<Bank>::open(&copy) else {
        panic!("the format is closed: a valid at_us does not excuse a retired member beside it");
    };

    assert!(
        refused.to_string().contains("unknown field `at`"),
        "the extra member is refused by name, never tolerated: {refused}"
    );

});
        }
    }

    {
        {
            let directory = scratch("crlf-wire");
            let copy = directory.join("journal.jsonl");
            std::fs::write(&copy, GOLDEN.replace('\n', "\r\n")).expect("the copy is writable");

            let Err(refused) = Jsonl::<Bank>::open(&copy) else {
                panic!("a CRLF copy must refuse, because forwarding it would rewrite its bytes");
            };
            assert!(refused.to_string().contains("carriage return"), "the refusal names the stray byte: {refused}");

            // The same answer when only the entries carry CR: the header parses, and
            // the first stored line refuses before the copy is opened for append.
            let mixed = directory.join("mixed.jsonl");
            std::fs::write(&mixed, GOLDEN.replace('\n', "\r\n").replacen("\r\n", "\n", 1)).expect("the copy is writable");
            let Err(refused) = Jsonl::<Bank>::open(&mixed) else {
                panic!("one CRLF line is enough to refuse the copy");
            };
            assert!(refused.to_string().contains("carriage return"), "the refusal names the stray byte: {refused}");
        }
    }
}

fn at_us(line: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(line).expect("a stored line is JSON")["metadata"]["at_us"]
        .as_i64()
        .expect("at_us is a signed integer")
}

fn micros() -> i64 {
    let since = Timestamp::now().duration_since(Timestamp::UNIX_EPOCH).expect("the test clock stands after the epoch");

    i64::try_from(since.as_micros()).expect("the test clock stands before the year 294247")
}
