/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../support/mod.rs"]
mod support;
#[path = "../support/vault.rs"]
mod vault;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
// Local Imports
use harmos_runtime::{
    DefinitionId, Identified, Runtime, Setup, StartupError, evidence::EvidenceError, evidence::EvidenceKind, evidence::Vault,
    services::Resident, services::ServiceContext, state::Position,
};
use support::{Bank, Copied, Ledger, Stored, catalog};
use vault::Directory;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Recording;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Upload {
    filename: String,
    content_type: String,
}

struct Report;

struct Rumour;

struct Interrupted {
    inner: Directory,
    at: Interruption,
}

struct Counted {
    inner: Directory,
    looks: watch::Sender<usize>,
}

#[derive(Clone)]
struct Idle;

/* === Enums === */

#[derive(Clone, Copy)]
enum Interruption {
    Verify,
    Fact,
    Claim,
}

/* === Traits === */

/* === Implementations === */

impl Identified for Recording {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("recording"), 1)
    }
}

impl EvidenceKind for Recording {
    type Details = Upload;
}

impl Identified for Report {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("report"), 1)
    }
}

impl EvidenceKind for Report {
    type Details = ();
}

impl Identified for Rumour {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("rumour"), 1)
    }
}

impl EvidenceKind for Rumour {
    type Details = ();
}

impl Interrupted {
    fn new(inner: Directory, at: Interruption) -> Self {
        Self { inner, at }
    }
}

impl Vault for Interrupted {
    fn ceiling(&self) -> usize {
        self.inner.ceiling()
    }

    fn cadence(&self) -> Duration {
        self.inner.cadence()
    }

    async fn open(&self, name: &str) -> Result<u64, EvidenceError> {
        self.inner.open(name).await
    }

    async fn external(&self, name: &str) -> Result<Option<PathBuf>, EvidenceError> {
        self.inner.external(name).await
    }

    async fn append(&self, name: &str, bytes: &[u8]) -> Result<u64, EvidenceError> {
        self.inner.append(name, bytes).await
    }

    async fn length(&self, name: &str) -> Result<u64, EvidenceError> {
        self.inner.length(name).await
    }

    async fn verify(&self, name: &str) -> Result<(u64, u64), EvidenceError> {
        match self.at {
            Interruption::Verify => Err(EvidenceError::unreadable("the disk went away")),
            // Durable and folded exactly as a real verify leaves them, and then
            // the process is over: the bytes survived, the fact was never
            // written.
            Interruption::Fact => {
                self.inner.verify(name).await?;
                Err(EvidenceError::unreadable("the process went away"))
            }
            Interruption::Claim => self.inner.verify(name).await,
        }
    }

    async fn claim(&self, name: &str) -> Result<(), EvidenceError> {
        match self.at {
            Interruption::Claim => Err(EvidenceError::unwritable("the disk went away")),
            Interruption::Verify | Interruption::Fact => self.inner.claim(name).await,
        }
    }

    async fn quarantine(&self, name: &str) -> Result<(), EvidenceError> {
        self.inner.quarantine(name).await
    }

    async fn read(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        self.inner.read(name, offset, limit).await
    }

    async fn read_draft(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        self.inner.read_draft(name, offset, limit).await
    }
}

impl Counted {
    fn new(inner: Directory) -> Self {
        Self {
            inner: inner.watching(Duration::from_millis(5)),
            looks: watch::Sender::new(0),
        }
    }

    fn looks(&self) -> watch::Receiver<usize> {
        self.looks.subscribe()
    }
}

impl Vault for Counted {
    fn ceiling(&self) -> usize {
        self.inner.ceiling()
    }

    fn cadence(&self) -> Duration {
        self.inner.cadence()
    }

    async fn open(&self, name: &str) -> Result<u64, EvidenceError> {
        self.inner.open(name).await
    }

    async fn external(&self, name: &str) -> Result<Option<PathBuf>, EvidenceError> {
        self.inner.external(name).await
    }

    async fn append(&self, name: &str, bytes: &[u8]) -> Result<u64, EvidenceError> {
        self.inner.append(name, bytes).await
    }

    async fn length(&self, name: &str) -> Result<u64, EvidenceError> {
        self.looks.send_modify(|looks| *looks += 1);
        self.inner.length(name).await
    }

    async fn verify(&self, name: &str) -> Result<(u64, u64), EvidenceError> {
        self.inner.verify(name).await
    }

    async fn claim(&self, name: &str) -> Result<(), EvidenceError> {
        self.inner.claim(name).await
    }

    async fn quarantine(&self, name: &str) -> Result<(), EvidenceError> {
        self.inner.quarantine(name).await
    }

    async fn read(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        self.inner.read(name, offset, limit).await
    }

    async fn read_draft(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        self.inner.read_draft(name, offset, limit).await
    }
}

impl Resident<Bank> for Idle {
    type Error = std::io::Error;

    async fn run(&mut self, mut context: ServiceContext<Bank>) -> Result<(), std::io::Error> {
        context.ready();
        context.stopping().await;

        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn upload() -> Upload {
    Upload {
        filename: "camera-3.mp4".to_owned(),
        content_type: "video/mp4".to_owned(),
    }
}

fn scratch(name: &str) -> Arc<tempfile::TempDir> {
    let prefix = format!("harmos-evidence-{name}-");
    let directory = tempfile::Builder::new().prefix(&prefix).tempdir().expect("the temporary directory is writable");
    Arc::new(directory)
}

fn declared(setup: &mut Setup<Bank>, vault: impl Vault) {
    setup.settings().register(catalog()).evidence(vault, [Recording::definition(), Report::definition()]);
}

async fn kept(vault: impl Vault) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        declared(setup, vault);
    })
    .await
    .expect("a fresh boot")
}

async fn kept_with(vault: impl Vault, history: &Stored) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        declared(setup, vault);
        setup.settings().recover(Copied::resuming(history));
    })
    .await
    .expect("the copy replays")
}

#[tokio::test]
async fn written_evidence_verified() {
    let runtime = kept(Directory::at(&scratch("written"))).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("session-7", "capture.log", upload()).await.expect("a declared kind opens");
    draft.write(b"first\n").await.expect("a draft takes bytes");
    draft.write(b"second\n").await.expect("a draft takes bytes");
    let recording = evidence.finalize(&draft).await.expect("verified bytes");

    assert_eq!(recording.length, 13);
    let finalized = recording.finalized.expect("finalizing records a fact");
    assert!(finalized.position > Position::ORIGIN, "the evidence is stamped with where its fact landed");

    let read = evidence.read::<Recording>("session-7", "capture.log", 0, 64).await.expect("a finalized evidence reads");
    assert_eq!(read.bytes, b"first\nsecond\n");
    assert_eq!(read.length, recording.length);
    assert_eq!(evidence.list("session-7"), vec![recording], "one finalized evidence, folded from the one fact that named it");
}

#[tokio::test]
async fn details_survive_reboot() {
    let directory = scratch("described");
    let history = Stored::default();
    {
        let runtime = kept_with(Directory::at(&directory), &history).await;
        let evidence = &runtime.evidence;
        let draft = evidence.open::<Recording>("session-7", "capture.log", upload()).await.expect("a declared kind opens");

        let open = evidence.list("session-7");
        let [unfinished] = open.as_slice() else {
            panic!("one open draft");
        };
        assert_eq!(
            unfinished.details::<Recording>().expect("the kind it was opened under"),
            upload(),
            "a draft describes itself from the call that named its kind"
        );

        draft.write(b"a capture").await.expect("bytes land");
        let recording = evidence.finalize(&draft).await.expect("verified bytes");

        assert_eq!(
            recording.details::<Recording>().expect("the kind it was opened under"),
            upload(),
            "what the draft was opened with is what the fact records"
        );
        runtime.shutdown().await.unwrap();
    }

    let reopened = kept_with(Directory::at(&directory), &history).await;
    let listed = reopened.evidence.list("session-7");
    let [recovered] = listed.as_slice() else {
        panic!("one finalized evidence");
    };

    assert_eq!(
        recovered.details::<Recording>().expect("the kind it was opened under"),
        upload(),
        "a boot folds the account out of the very fact it folds the evidence from"
    );
    assert!(
        matches!(recovered.details::<Report>(), Err(EvidenceError::Mismatched { .. })),
        "an account means what its own declaration says, and no other kind may read it"
    );
}

#[tokio::test]
async fn unnamed_bytes_invisible() {
    let directory = scratch("crashed");
    let history = Stored::default();
    {
        let runtime = kept_with(Interrupted::new(Directory::at(&directory), Interruption::Fact), &history).await;
        let draft = runtime.evidence.open::<Recording>("session-7", "capture.log", upload()).await.expect("a declared kind opens");
        draft.write(b"half a capture").await.expect("bytes land");

        // The bytes are made durable and read back, and the process ends in
        // the instant before the fact naming them would have been recorded.
        let refused = runtime.evidence.finalize(&draft).await.expect_err("the crash falls between the durable bytes and the fact");
        assert!(matches!(refused, EvidenceError::Unreadable(_)));
        runtime.shutdown().await.unwrap();
    }

    let reopened = kept_with(Directory::at(&directory), &history).await;
    let evidence = &reopened.evidence;

    assert!(evidence.list("session-7").is_empty(), "recovery reads facts, and no fact names these bytes");
    assert_eq!(
        evidence.read::<Recording>("session-7", "capture.log", 0, 64).await.expect_err("nothing finalized under this key"),
        EvidenceError::Unknown
    );

    let resumed =
        evidence.open::<Recording>("session-7", "capture.log", upload()).await.expect("an unfinished draft is resumed, not replaced");

    assert_eq!(resumed.length(), 14, "the staged bytes are still there to carry on from");
}

#[tokio::test]
async fn interrupted_claim_heals() {
    let directory = scratch("healed");
    let history = Stored::default();
    let vault = Interrupted::new(Directory::at(&directory), Interruption::Claim);
    {
        let runtime = kept_with(vault, &history).await;
        let draft = runtime.evidence.open::<Recording>("session-7", "capture.log", upload()).await.expect("a declared kind opens");
        draft.write(b"whole capture").await.expect("bytes land");

        let refused = runtime.evidence.finalize(&draft).await.expect_err("the claim is interrupted");
        assert!(matches!(refused, EvidenceError::Unwritable(_)));

        assert_eq!(runtime.evidence.list("session-7").len(), 1, "the fact landed before the claim, so the evidence is already true");
        runtime.shutdown().await.unwrap();
    }

    let reopened = kept_with(Directory::at(&directory), &history).await;
    let listed = reopened.evidence.list("session-7");

    assert_eq!(listed.len(), 1, "the fact folded back on its own");
    assert!(listed[0].finalized.is_some());
    assert_eq!(
        reopened
            .evidence
            .read::<Recording>("session-7", "capture.log", 0, 64)
            .await
            .expect("the interrupted claim is completed on the way in")
            .bytes,
        b"whole capture",
    );
}

#[tokio::test]
async fn bounded_read_clamps() {
    let runtime = kept(Directory::at(&scratch("bounded")).reading(4)).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("run-1", "tail.log", upload()).await.expect("a declared kind opens");
    draft.write(b"abcdefg").await.expect("bytes land");

    let window = evidence.read::<Recording>("run-1", "tail.log", 2, 4).await.expect("a live draft reads");
    assert_eq!(window.bytes, b"cdef");
    assert_eq!(window.length, 7);
    assert_eq!(window.next(), 6);

    let past = evidence.read::<Recording>("run-1", "tail.log", 99, 4).await.expect("reading past the end is not a failure");
    assert_eq!(past.requested, 99);
    assert_eq!(past.offset, 7, "the offset clamps to what is there");
    assert!(past.bytes.is_empty());
    assert_eq!(past.next(), 7);

    assert_eq!(
        evidence.read::<Recording>("run-1", "tail.log", 0, 8).await.expect_err("past the vault's own bound"),
        EvidenceError::Bound { requested: 8, ceiling: 4 },
        "a bound a caller got wrong is refused, never quietly shortened"
    );

    evidence.finalize(&draft).await.expect("verified bytes");

    let finished = evidence.read::<Recording>("run-1", "tail.log", 5, 4).await.expect("the same key reads after it finalizes");
    assert_eq!(finished.bytes, b"fg");
    assert_eq!(finished.length, 7);
    assert_eq!(finished.next(), 7);
}

#[tokio::test]
async fn scope_finalizes_all() {
    let runtime = kept(Directory::at(&scratch("scoped"))).await;
    let evidence = &runtime.evidence;

    for key in ["one.log", "two.log"] {
        let draft = evidence.open::<Recording>("run-9", key, upload()).await.expect("a declared kind opens");
        draft.write(key.as_bytes()).await.expect("bytes land");
    }
    let report = evidence.open::<Report>("run-9", "one.log", ()).await.expect("a key is unique within its kind, not across kinds");
    report.write(b"summary").await.expect("bytes land");

    let before = evidence.list("run-9");
    assert_eq!(before.len(), 3);
    assert!(before.iter().all(|evidence| evidence.finalized.is_none()));

    let finalized = evidence.finalize_scope("run-9").await.expect("every draft in the scope");
    assert_eq!(finalized.len(), 3);

    let after = evidence.list("run-9");
    assert_eq!(
        after.len(),
        3,
        "the union of active and finalized never gains or loses an entry: the \
         key is the identity on both sides of finalization"
    );
    assert!(after.iter().all(|evidence| evidence.finalized.is_some()));
    assert_eq!(
        after.iter().map(|evidence| (evidence.kind.to_string(), evidence.key.clone())).collect::<Vec<_>>(),
        vec![
            ("recording".to_owned(), "one.log".to_owned()),
            ("recording".to_owned(), "two.log".to_owned()),
            ("report".to_owned(), "one.log".to_owned()),
        ]
    );
}

#[tokio::test]
async fn watch_coalesces_final() {
    let runtime = kept(Directory::at(&scratch("watched"))).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("run-4", "growing.log", upload()).await.expect("a declared kind opens");
    let mut watching = evidence.watch(&draft).expect("a live draft is watchable");
    assert_eq!(*watching.borrow_and_update(), 0);

    draft.write(b"ab").await.expect("bytes land");
    draft.write(b"cde").await.expect("bytes land");
    watching.changed().await.expect("growth wakes a watcher");
    assert_eq!(*watching.borrow_and_update(), 5, "two appends coalesce into the one length a watcher has yet to see");

    evidence.finalize(&draft).await.expect("verified bytes");
    watching.changed().await.expect("the final revision precedes the close");
    assert_eq!(*watching.borrow_and_update(), 5);
    assert!(watching.changed().await.is_err(), "the channel closing is how a watcher learns there will be no more");
}

// The vault's looking cadence is a Tokio timer and the directory vault does
// synchronous file I/O, so the paused clock steps the cadence exactly.
#[tokio::test(start_paused = true)]
async fn watch_drives_looking() {
    let counted = Counted::new(Directory::at(&scratch("unwatched")));
    let mut looks = counted.looks();
    let runtime = kept(counted).await;
    let evidence = &runtime.evidence;

    let draft = evidence.external::<Recording>("run-5", "quiet.log", upload()).await.expect("a filesystem vault hands out a path");
    tokio::time::advance(Duration::from_millis(60)).await;

    assert_eq!(*looks.borrow(), 0, "nobody is watching, so nothing polls");

    let watching = evidence.watch(&draft).expect("a live draft is watchable");
    looks.wait_for(|looks| *looks > 0).await.expect("a watcher is what starts the looking");

    drop(watching);
    tokio::time::advance(Duration::from_millis(60)).await;
    let settled = *looks.borrow();
    tokio::time::advance(Duration::from_millis(60)).await;

    assert_eq!(*looks.borrow(), settled, "the last watcher leaving is what stops it again");
}

#[tokio::test]
async fn unverified_draft_survives() {
    let directory = scratch("unverified");
    let runtime = kept(Interrupted::new(Directory::at(&directory), Interruption::Verify)).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("run-6", "torn.log", upload()).await.expect("a declared kind opens");
    draft.write(b"suspect").await.expect("bytes land");

    let refused = evidence.finalize(&draft).await.expect_err("the bytes do not verify");
    assert!(matches!(refused, EvidenceError::Unreadable(_)));

    assert_eq!(evidence.list("run-6").len(), 1, "the draft outlived the attempt to finalize it");
    assert!(evidence.list("run-6")[0].finalized.is_none(), "no fact was recorded, so nothing is finalized");
}

#[tokio::test]
async fn abandoned_draft_quarantined() {
    let directory = scratch("abandoned");
    let runtime = kept(Directory::at(&directory)).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("run-7", "partial.log", upload()).await.expect("a declared kind opens");
    draft.write(b"as far as it got").await.expect("bytes land");
    evidence.abandon(&draft).await.expect("a draft is put aside");

    assert!(evidence.list("run-7").is_empty());
    assert_eq!(draft.write(b"more").await.expect_err("the draft has ended"), EvidenceError::Unknown);
    assert_eq!(Directory::at(&directory).quarantined().len(), 1, "a partial capture is evidence of what happened, so it is kept");
}

#[tokio::test]
async fn key_claimed_once() {
    let runtime = kept(Directory::at(&scratch("claimed"))).await;
    let evidence = &runtime.evidence;

    let draft = evidence.open::<Recording>("run-8", "once.log", upload()).await.expect("a declared kind opens");

    assert!(matches!(
        evidence.open::<Recording>("run-8", "once.log", upload()).await.expect_err("the key is taken"),
        EvidenceError::Conflict { .. }
    ));

    evidence.finalize(&draft).await.expect("verified bytes");

    assert!(
        matches!(
            evidence.open::<Recording>("run-8", "once.log", upload()).await.expect_err("evidence is never overwritten"),
            EvidenceError::Conflict { .. }
        ),
        "a finalized key stays claimed"
    );
    assert_eq!(
        evidence.open::<Rumour>("run-8", "hearsay.log", ()).await.expect_err("outside the frozen catalog"),
        EvidenceError::Undeclared { kind: DefinitionId::new("rumour") }
    );
}

#[tokio::test]
async fn undeclared_vault_unserved() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
    })
    .await
    .expect("a fresh boot");

    assert_eq!(
        runtime.evidence.open::<Recording>("run-0", "nowhere.log", upload()).await.expect_err("nothing was declared to keep it in"),
        EvidenceError::Unserved
    );
    assert!(runtime.evidence.list("run-0").is_empty());
}

#[tokio::test]
async fn artifact_service_refused() {
    let refused = refusal(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(catalog());
            setup.settings().service("artifact", Idle);
        })
        .await,
    );

    assert!(refused.to_string().contains("artifact"), "one fact namespace, one author: {refused}");
}

#[tokio::test]
async fn duplicate_kind_refused() {
    let refused = refusal(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(catalog());
            setup.settings().evidence(Directory::at(&scratch("twice")), [Recording::definition(), Recording::definition()]);
        })
        .await,
    );

    assert!(refused.to_string().contains("recording"), "{refused}");
}

fn refusal(booted: Result<Runtime<Bank>, StartupError>) -> StartupError {
    match booted {
        Ok(_) => panic!("this build was never meant to stand up"),
        Err(refused) => refused,
    }
}

/* === Tests === */
