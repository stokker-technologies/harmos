/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn corrupt_snapshot_blocks() {
    for field in ["schema", "position", "undo", "receipts", "tail", "boundary"] {
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = launch(policy(100), &storage).await;
        runtime.state.as_principal(PrincipalId::new("author")).commit_keyed(RequestKey::new("key"), deposit(4)).await.unwrap();
        crate::support::app::shutdown(&runtime).await.unwrap();
        {
            let mut snapshot = store.recovery.snapshot.lock().unwrap();
            let snapshot = snapshot.as_mut().unwrap();
            let mut value: serde_json::Value = serde_json::from_reader(snapshot.payload.reader().unwrap()).unwrap();
            match field {
                "schema" => value["schema"]["id"] = "other".into(),
                "position" => value["position"] = 0.into(),
                "undo" => value["undos"][0]["inverse"]["id"] = "unknown".into(),
                "receipts" => {
                    let duplicate = value["receipts"][0].clone();
                    value["receipts"].as_array_mut().unwrap().push(duplicate);
                }
                "tail" => value["tail"][0]["position"] = 2.into(),
                "boundary" => snapshot.through.sequence += 1,
                _ => unreachable!(),
            }
            snapshot.payload = SnapshotPayload::from_bytes(
                &serde_json::to_vec(&value).unwrap(),
                policy(100).max_bytes,
                snapshot.payload.manifest().position.unwrap(),
            )
            .unwrap();
        }
        let storage = store.manager(config());
        assert!(
            crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
                setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                    replay: limits(),
                    capture: limits(),
                    snapshots: Some(policy(100)),
                    journal_tail: 2,
                });
            })
            .await
            .is_err(),
            "{field}"
        );
        assert!(storage.status().closed);
        assert!(!store.data.control.open.load(Ordering::SeqCst));
        assert!(!store.recovery.control.open.load(Ordering::SeqCst));
    }
}
