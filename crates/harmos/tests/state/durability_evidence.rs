/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::path::PathBuf;
// Third Party Imports

// Local Imports
use harmos::evidence::Vault;
use jsonl_adapter::EvidenceDirectory;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("evidence")
        .join(name);
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the target directory is writable");

    directory
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("bytes_survives_process", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("claimed");
                    let digest = {
                        let vault = EvidenceDirectory::at(&root).expect("a fresh directory");
                        vault
                            .open("recording~run~one")
                            .await
                            .expect("a fresh draft");
                        vault
                            .append("recording~run~one", b"evidence")
                            .await
                            .expect("a draft takes bytes");

                        let (digest, length) =
                            vault.verify("recording~run~one").await.expect("readable");
                        assert_eq!(length, 8, "the length is measured from what is stored");
                        vault.claim("recording~run~one").await.expect("named");

                        digest
                    };

                    let reopened =
                        EvidenceDirectory::at(&root).expect("a directory a stop left behind");
                    let (bytes, length) = reopened
                        .read("recording~run~one", 0, 64)
                        .await
                        .expect("kept evidence reads");

                    assert_eq!(bytes, b"evidence");
                    assert_eq!(length, 8);
                    assert!(
                        root.join("kept").join("recording~run~one").exists(),
                        "a claim is a rename into the kept directory, never a copy over a \
         path a reader is already looking at"
                    );
                    assert!(!root.join("drafts").join("recording~run~one").exists());
                    assert!(
                        reopened.open("recording~run~one").await.is_err(),
                        "a kept name is never reopened as a draft: {digest:x}"
                    );
                });
        });
    }
    {
        scenario_runtime::check("digest_answers_nothing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("digested");
                    let vault = EvidenceDirectory::at(&root).expect("a fresh directory");
                    vault.open("log~run~one").await.expect("a fresh draft");
                    vault
                        .append("log~run~one", b"as written")
                        .await
                        .expect("a draft takes bytes");
                    let (honest, _) = vault.verify("log~run~one").await.expect("readable");

                    // Something outside this runtime rewrites the staged bytes in place. The
                    // producer's account of what it wrote is unchanged; the stored bytes are
                    // not, and only the stored bytes are ever folded.
                    std::fs::write(root.join("drafts").join("log~run~one"), b"as changed")
                        .expect("the staged file is writable");
                    let (found, length) = vault.verify("log~run~one").await.expect("readable");

                    assert_ne!(honest, found);
                    assert_eq!(length, 10);
                });
        });
    }
    {
        scenario_runtime::check("interrupted_claim_completed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("healed");
                    let vault = EvidenceDirectory::at(&root).expect("a fresh directory");
                    vault.open("log~run~two").await.expect("a fresh draft");
                    vault
                        .append("log~run~two", b"already true")
                        .await
                        .expect("a draft takes bytes");
                    vault.verify("log~run~two").await.expect("readable");

                    // The fact naming this evidence is durable before the claim ever runs, so
                    // bytes still staged under a finalized name are bookkeeping catching up.
                    let (bytes, length) = vault
                        .read("log~run~two", 0, 64)
                        .await
                        .expect("the kept read completes the claim on the way in");

                    assert_eq!(bytes, b"already true");
                    assert_eq!(length, 12);
                    assert!(root.join("kept").join("log~run~two").exists());
                    assert!(!root.join("drafts").join("log~run~two").exists());
                });
        });
    }
    {
        scenario_runtime::check("abandoned_draft_moved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("quarantined");
                    let vault = EvidenceDirectory::at(&root).expect("a fresh directory");

                    for run in ["one", "two"] {
                        vault.open("log~run~same").await.expect("a fresh draft");
                        vault
                            .append("log~run~same", run.as_bytes())
                            .await
                            .expect("a draft takes bytes");
                        vault.quarantine("log~run~same").await.expect("put aside");
                    }

                    assert_eq!(
                        vault.quarantined().expect("the quarantine is readable"),
                        ["log~run~same", "log~run~same.1"],
                        "a second abandoned draft under one name never overwrites the first: \
         each is the only account of its own interrupted write"
                    );
                    assert!(
                        vault.quarantine("log~run~never").await.is_ok(),
                        "abandoning nothing is not a failure"
                    );
                });
        });
    }
    {
        scenario_runtime::check("read_bounded_directorys", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let vault = EvidenceDirectory::at(scratch("bounded"))
                        .expect("a fresh directory")
                        .reading(4);
                    vault.open("log~run~long").await.expect("a fresh draft");
                    vault
                        .append("log~run~long", b"abcdefghij")
                        .await
                        .expect("a draft takes bytes");

                    let (bytes, length) = vault
                        .read_draft("log~run~long", 0, 64)
                        .await
                        .expect("a staged draft reads");

                    assert_eq!(
                        bytes, b"abcd",
                        "the backend never answers past its own bound"
                    );
                    assert_eq!(length, 10, "and says how much more there is");

                    let (past, _) = vault
                        .read_draft("log~run~long", 99, 4)
                        .await
                        .expect("reading past the end is not a failure");
                    assert!(past.is_empty());
                });
        });
    }
    {
        scenario_runtime::check("external_path_exists", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("external");
                    let vault = EvidenceDirectory::at(&root).expect("a fresh directory");
                    vault.open("log~run~child").await.expect("a fresh draft");

                    let path = vault
                        .external("log~run~child")
                        .await
                        .expect("a filesystem vault has a path")
                        .expect("and hands it out");

                    assert!(path.exists(), "a child is handed a file, not a promise");
                    assert_eq!(path, root.join("drafts").join("log~run~child"));

                    std::fs::write(&path, b"written outside").expect("the path is writable");

                    assert_eq!(
                        vault.length("log~run~child").await.expect("readable"),
                        15,
                        "growth a foreign writer made is what looking finds"
                    );
                });
        });
    }
    {
        scenario_runtime::check("resumed_draft_answers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = scratch("resumed");
                    {
                        let vault = EvidenceDirectory::at(&root).expect("a fresh directory");
                        vault.open("log~run~half").await.expect("a fresh draft");
                        vault
                            .append("log~run~half", b"half")
                            .await
                            .expect("a draft takes bytes");
                    }

                    let reopened =
                        EvidenceDirectory::at(&root).expect("a directory a stop left behind");

                    assert_eq!(
                        reopened.open("log~run~half").await.expect("a staged draft"),
                        4,
                        "an unfinished draft is resumed rather than replaced"
                    );
                    assert_eq!(
                        reopened
                            .append("log~run~half", b" more")
                            .await
                            .expect("a resumed draft carries on"),
                        9
                    );
                    assert!(
                        root.join("drafts").join("log~run~half").exists(),
                        "nothing promotes a draft but the claim that follows a durable fact"
                    );
                });
        });
    }
}

/* === Tests === */
