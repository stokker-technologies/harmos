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
use crate::backend::storage_settings;
// Standard Library Imports
use std::fmt;
// Third Party Imports

// Local Imports
use super::*;
use harmos::storage::{ObjectKey, ObjectQuery};

/* === Constants === */
const CASE: &str = "charger:0123456789";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("punctuation_roundtrips_point", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(storage_settings()).await;
                    let key = CASE.to_owned();
                    let wanted = Document {
                        id: key.clone(),
                        status: "ready".into(),
                    };
                    let receipt = f.storage.write_object(&wanted).await.unwrap();
                    for id in ["Charger:0123456789", "../calibration:1"] {
                        f.storage
                            .write_object(&Document {
                                id: id.into(),
                                status: "other".into(),
                            })
                            .await
                            .unwrap();
                    }
                    let saved = f.storage.flush().await.unwrap();
                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    assert_eq!(
                        *f.primary.objects.lock().unwrap(),
                        *f.replica.objects.lock().unwrap()
                    );
                    for version in [
                        ObjectVersion::Latest,
                        ObjectVersion::Exact(receipt.version.clone()),
                    ] {
                        let found = f
                            .storage
                            .read_object::<Document>(&key, version, limits().bytes)
                            .await
                            .unwrap()
                            .unwrap();
                        assert_eq!(found.value, wanted);
                        assert_eq!(found.version, receipt.version);
                    }

                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let found = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &key,
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(found.value.id, CASE);
                    let mut typed = revisions();
                    typed.item = Some(&key);
                    let mut pages = f.storage.query_in(&view, typed).unwrap();
                    let page = pages.next_page().await.unwrap().unwrap();
                    assert_eq!(page.len(), 1);
                    assert_eq!(page[0].value, wanted);
                    assert_eq!(page[0].value.id, CASE);
                    assert!(pages.next_page().await.unwrap().is_none());
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key,
                                ObjectVersion::Latest,
                                NonZeroUsize::MIN
                            )
                            .await
                            .is_err()
                    );
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("keys_refuse_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let primary = Arc::new(Data::<Objects>::with_catalog(history()));
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: primary.clone(),
                            state: Arc::new(Recovery::new(history())),
                            replicas: vec![],
                        },
                        StorageSettings {
                            request: {
                                let mut settings = storage_settings().request;
                                settings.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
                                settings
                            },
                            target_batch_bytes: NonZeroUsize::new(2048).unwrap(),
                            max_batch_bytes: NonZeroUsize::new(4096).unwrap(),
                            ..storage_settings()
                        },
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    let before = storage.cursor_accepted();
                    let large = Document {
                        id: "x".repeat(8193),
                        status: "ready".into(),
                    };
                    assert!(storage.write_object(&large).await.is_err());
                    assert_eq!(storage.cursor_accepted(), before);
                    assert!(primary.objects.lock().unwrap().is_empty());
                    assert!(
                        storage
                            .read_object::<Document>(
                                &large.id,
                                ObjectVersion::Latest,
                                limits().bytes
                            )
                            .await
                            .is_err()
                    );

                    let mut typed = ObjectQuery::<Document>::new();
                    typed.item = Some(&large.id);
                    typed.options.page = Some(limits());
                    assert!(storage.query(typed).await.is_err());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("keys_preserve_text", || {
            for text in [
                CASE,
                "Charger:0123456789",
                "../calibration:1",
                " key with spaces ",
                "é",
            ] {
                let key = ObjectKey::new(text, NonZeroUsize::new(text.len()).unwrap()).unwrap();
                assert_eq!(key.as_str(), text);
                assert_eq!(key.to_string(), text);
                let encoded = serde_json::to_string(&key).unwrap();
                assert_eq!(serde_json::from_str::<ObjectKey>(&encoded).unwrap(), key);
                assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), text);
            }
            assert!(Identifier::new(CASE).is_err());
            assert!(ObjectKey::new("", limits().bytes).is_err());
            assert!(ObjectKey::new("é", NonZeroUsize::MIN).is_err());
            assert!(serde_json::from_str::<ObjectKey>("\"\"").is_err());

            struct Segments;
            impl fmt::Display for Segments {
                fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                    out.write_str("1234")?;
                    out.write_str("5678")?;
                    panic!("formatting must stop before the next segment");
                }
            }
            assert!(ObjectKey::new(&Segments, NonZeroUsize::new(5).unwrap()).is_err());
        });
    }
    {
        scenario_runtime::check("catalog_rejects_changed", || {
            let value = Document {
                id: CASE.into(),
                status: "ready".into(),
            };
            let mut stored = Objects::encode(&value, limits().bytes).unwrap();
            assert_eq!(stored.item.as_str(), CASE);
            assert_eq!(
                Objects::decode::<Document>(&stored, limits().bytes).unwrap(),
                value
            );
            stored.item = ObjectKey::new("Charger:0123456789", limits().bytes).unwrap();
            assert!(Objects::decode::<Document>(&stored, limits().bytes).is_err());
            assert!(Objects::validate_value(&stored, limits().bytes).is_err());
            stored.item =
                ObjectKey::new(&"x".repeat(4097), NonZeroUsize::new(4097).unwrap()).unwrap();
            assert!(Objects::decode::<Document>(&stored, limits().bytes).is_err());
        });
    }
}

/* === Tests === */
