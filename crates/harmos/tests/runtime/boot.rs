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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::probe::{Change, EXPORTED, Editor, Note, RENAME, Sheet, inverses, named, rename};
use harmos::state::{
    CorrelationId, DecodeError, Entry, Envelope, Focus, Kind, Link, Position, PrincipalId,
    Recorded, Timestamp,
};
use harmos::storage::{JournalConsumer, Payload, Recovery, Source};
use harmos::{DefinitionId, Runtime, runtime::Catalog, runtime::Incompatible, runtime::Registration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
struct Definitions(Vec<(Kind, DefinitionId, u32)>);

struct Replaying {
    recorded: Vec<Envelope<Payload<Editor>>>,
    resume: Position,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Registration<Editor> for Definitions {
    fn register(self) -> Catalog<Editor> {
        fn decode(
            _id: &DefinitionId,
            _version: u32,
            _bytes: &[u8],
        ) -> Result<Option<Change>, DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: self.0,
            decode,
            inverses: Some(inverses),
        }
    }
}

impl JournalConsumer<Editor> for Replaying {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Editor>>>>,
        applied: Position,
    ) -> Position {
        self.resume = applied;

        applied
    }
}

impl Source<Editor> for Replaying {
    async fn load(&mut self) -> Result<Recovery<Editor>, Incompatible> {
        let entries = std::mem::take(&mut self.recorded);
        self.resume = entries
            .last()
            .map_or(Position::ORIGIN, |entry| entry.position);

        Ok(Recovery {
            commits: Vec::new(),
            frontier: self.resume,
            entries,
            encoded: Vec::new(),
        })
    }
}

/* === Public Functions === */
pub(crate) fn catalog() -> impl Registration<Editor> {
    Definitions([named(RENAME), named(EXPORTED)].into())
}

/* === Private Functions === */
fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

fn origin() -> Sheet {
    Sheet {
        title: "origin".to_owned(),
        sealed: false,
    }
}

fn stored(
    position: u64,
    id: &str,
    link: Option<Link>,
    value: Recorded<Editor>,
) -> Envelope<Payload<Editor>> {
    Envelope {
        position: Position(position),
        kind: value.kind(),
        id: DefinitionId::new(id),
        version: 1,
        metadata: serde_json::from_value(serde_json::json!({
            "commit": null,
            "principal": alice(),
            "correlation": CorrelationId(Position(position)),
            "causation": null,
            "request": null,
            "link": link,
            "at": Timestamp::UNIX_EPOCH,
        }))
        .unwrap(),
        payload: Payload {
            prepared: None,
            id: DefinitionId::new(id),
            version: 1,
            value,
            apply: None,
        },
    }
}

fn renamed(position: u64, title: &str, link: Option<Link>) -> Envelope<Payload<Editor>> {
    stored(
        position,
        RENAME.1,
        link,
        Recorded::Transaction(Arc::new(rename(title))),
    )
}

fn recovering(recorded: Vec<Envelope<Payload<Editor>>>) -> Replaying {
    Replaying {
        recorded,
        resume: Position::ORIGIN,
    }
}

async fn title(runtime: &Runtime<Editor>) -> String {
    runtime
        .state
        .state()
        .read(|sheet| sheet.title.clone())
        .await
        .expect("reads are open")
        .value
}

/* === Tests === */

#[tokio::test]
async fn boot_preserves_origin() {
    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
        .register(catalog())
        .runtime()
        .await
        .expect("nothing was recovered");

    assert_eq!(title(&runtime).await, "");
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    assert_eq!(
        runtime.state.stored(),
        runtime.state.applied(),
        "with nothing attached, stored tracks applied"
    );
    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn read_stamped_position() {
    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
        .register(catalog())
        .runtime()
        .await
        .expect("nothing was recovered");

    let receipt = runtime
        .state
        .commit(rename("draft"))
        .await
        .expect("accepted");
    let read = runtime
        .state
        .state()
        .read(|sheet| sheet.title.clone())
        .await
        .expect("reads are open");

    assert_eq!(read.value, "draft");
    assert_eq!(read.position, receipt.position);
    assert!(read.position >= receipt.position);
}

#[tokio::test]
async fn boot_folds_state() {
    // The live path: two renames and one undo of the second.
    let live = Runtime::<Editor>::builder(origin(), ())
        .register(catalog())
        .runtime()
        .await
        .expect("nothing was recovered");
    let handle = live.state.as_principal(alice());
    handle.commit(rename("first")).await.expect("accepted");
    let second = handle.commit(rename("second")).await.expect("accepted");
    handle
        .undo(alice(), Focus::all())
        .await
        .expect("alice has an entry to undo");

    // The recovered path: the very same entries, replayed.
    let recovered = Runtime::<Editor>::builder(origin(), ())
        .register(catalog())
        .recover(recovering(vec![
            renamed(1, "first", None),
            renamed(2, "second", None),
            renamed(3, "first", Some(Link::UndoOf(second.position))),
            stored(4, EXPORTED.1, None, Recorded::Record(Note::Exported)),
        ]))
        .runtime()
        .await
        .expect("every definition is declared");

    assert_eq!(title(&live).await, title(&recovered).await);
    assert_eq!(recovered.state.applied(), Position(4));

    // The rebuilt fold has to answer the next undo exactly as the live one
    // does — which it can only do if replay re-derived the inverse of entry
    // one against the state as it stood before entry one.
    live.state
        .undo(alice(), Focus::all())
        .await
        .expect("the first rename is still active");
    recovered
        .state
        .undo(alice(), Focus::all())
        .await
        .expect("the rebuilt stack knows the same");

    assert_eq!(title(&live).await, "origin");
    assert_eq!(
        title(&recovered).await,
        "origin",
        "the inverse was re-derived against the state before its own entry"
    );
}

#[tokio::test]
async fn boot_refuses_loudly() {
    let booted = Runtime::<Editor>::builder(Sheet::default(), ())
        .register(catalog())
        .recover(recovering(vec![stored(
            1,
            "sheet.rotate",
            None,
            Recorded::Transaction(Arc::new(Change::Seal)),
        )]))
        .runtime()
        .await;

    let Err(refused) = booted else {
        panic!("no definition is declared under that name");
    };
    assert!(
        refused.to_string().contains("sheet.rotate"),
        "the refusal names what it could not honor: {refused}"
    );
}
