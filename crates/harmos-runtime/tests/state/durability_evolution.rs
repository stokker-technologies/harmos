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
use std::sync::{Arc, Mutex};
// Third Party Imports
use support::{Bank, Bonus, Fact, Ledger, Noted, booted, deposit, ledger};
// Local Imports
use super::support;
use harmos_runtime::{state::Entry, state::Kind, state::Origin, state::Position, state::Recorded, storage::JournalConsumer};
use support::journal::{Journal, Store, frame};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Held = Arc<Mutex<Vec<Arc<Entry<Recorded<Bank>>>>>>;

/* === Structs === */
struct Kept {
    entries: Held,
    resume: Position,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Kept {
    fn new() -> (Self, Held) {
        let entries: Held = Arc::new(Mutex::new(Vec::new()));

        (Self { entries: Arc::clone(&entries), resume: Position::ORIGIN }, entries)
    }
}

impl JournalConsumer<Bank> for Kept {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<Bank>>>>, applied: Position) -> Position {
        let mut held = self.entries.lock().expect("the fold is never poisoned");

        for entry in entries {
            self.resume = entry.position;
            held.push(entry);
        }

        applied.min(self.resume)
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn replay_applies_version() {
    let copy = Store::default();

    // What this build writes: a bonus of 10 credits 10.
    let live = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    let written = live.state.commit(Bonus { amount: 10 }).await.expect("accepted");
    live.state.wait_until_stored(written.position).await;
    live.stop().await.unwrap();

    // What a version-1 build wrote: the same declared amount, under version 1,
    // in version 1's own shape — and version 1 credited double.
    copy.push(frame(2, Kind::Transaction, "ledger.bonus", 1, serde_json::json!({ "amount": 10 })));

    let recovered = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    let rebuilt = ledger(&recovered).await;

    assert_eq!(
        rebuilt.balance, 30,
        "the entry this build wrote credited 10 and the one version 1 wrote \
         credited 20, because replay asked the version each was written with"
    );
    assert_eq!(rebuilt.applications, 2);
    assert_eq!(recovered.state.applied(), Position(2));
    let historical = recovered
        .state
        .state()
        .at(Origin::initial(Ledger::default()), Position(2))
        .await
        .expect("the stored versions replay historically too");
    assert_eq!(historical.value.balance, 30);
    recovered.stop().await.unwrap();
}

#[tokio::test]
async fn stored_fact_upgrades() {
    let copy = Store::default();

    let live = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    let written = live.state.commit(deposit(4)).await.expect("accepted");
    live.state.wait_until_stored(written.position).await;
    live.stop().await.unwrap();

    // A fact as a version-1 build wrote it: the note under a field name the
    // current shape does not have.
    copy.push(frame(2, Kind::Record, "ledger.noted", 1, serde_json::json!({ "text": "audited" })));

    let (kept, held) = Kept::new();
    let recovered = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy)).attach(kept)).await;

    assert_eq!(recovered.state.applied(), Position(2));

    // The fold is drained by the supervisor, so what it was handed is settled
    // once the runtime has stopped.
    recovered.stop().await.unwrap();

    assert_eq!(
        facts(&held),
        vec![Fact::Noted(Noted("audited".to_owned()))],
        "an older fact converts into the current type and is nothing else \
         afterwards, which is what a fold below it is handed"
    );
}

fn facts(held: &Held) -> Vec<Fact> {
    held.lock()
        .expect("the fold is never poisoned")
        .iter()
        .filter_map(|entry| match &entry.value {
            Recorded::Record(fact) => Some(fact.clone()),
            Recorded::Transaction(_) => None,
            Recorded::System(_) => None,
        })
        .collect()
}

/* === Tests === */
