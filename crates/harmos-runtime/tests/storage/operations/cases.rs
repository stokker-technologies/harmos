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

#[path = "deletion.rs"]
mod deletion;
#[path = "files.rs"]
mod files;
#[path = "objects.rs"]
mod objects;
#[cfg(feature = "plugin")]
#[path = "plugins.rs"]
mod plugins;
#[path = "queries.rs"]
mod queries;
#[path = "schemas.rs"]
mod schemas;
#[path = "writes.rs"]
mod writes;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Fixture, config, write};
use harmos_runtime::operations::Contracts;
use harmos_runtime::storage::operations::Operations;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos_runtime::lanes]
pub(super) enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn followed(
    item: harmos_runtime::operations::StreamItem<harmos_runtime::storage::operations::Followed>,
) -> harmos_runtime::storage::StoredRecord {
    match &*item {
        harmos_runtime::storage::operations::Followed::Record(record) => record.clone(),
        harmos_runtime::storage::operations::Followed::Boundaries(_) => {
            panic!("boundaries are reported once, before records")
        }
    }
}

fn records() -> harmos_runtime::storage::query::RecordSelection {
    harmos_runtime::storage::query::RecordSelection {
        schema: super::schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    }
}

/* === Tests === */

fn own_contract() {
    {
        scenario_runtime::check("catalog_exposes_save", || {
            let contracts = Contracts::of::<Operations>().unwrap();
            let served: Vec<_> = contracts
                .iter()
                .filter(|contract| contract.grpc().is_some())
                .map(|contract| contract.operation().id.to_string())
                .collect();
            for required in [
                "storage.query",
                "storage.read_file",
                "storage.write_records",
                "storage.write_streams",
                "storage.write_objects",
                "storage.write_file",
            ] {
                assert!(served.iter().any(|id| id == required), "missing {required}");
            }
            for retired in [
                "storage.read_records",
                "storage.read_objects",
                "storage.query_stream",
                "storage.read_files",
                "storage.reductions",
            ] {
                assert!(!served.iter().any(|id| id == retired), "retired {retired}");
            }
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("plugin_facing_storage", || {
            let contracts = Contracts::of::<Operations>()
                .unwrap()
                .into_iter()
                .filter(|contract| contract.grpc().is_some())
                .collect::<Vec<_>>();
            let artifacts = harmos_packaging::proto::Artifacts::new(&contracts).unwrap();
            let target = tempfile::tempdir().unwrap();
            let root =
                harmos_packaging::proto::generate(&contracts, &target.path().join("grpc/storage"))
                    .unwrap();
            let output =
                std::process::Command::new(protoc_bin_vendored::protoc_bin_path().unwrap())
                    .current_dir(&root)
                    .arg("--proto_path=.")
                    .arg(format!(
                        "--descriptor_set_out={}",
                        target.path().join("compiled.bin").display()
                    ))
                    .args(artifacts.files().keys())
                    .output()
                    .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        });
    }
    {
        scenario_runtime::check("crosses_save_resumes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos_runtime::storage::operations::{FollowRecords, Followed};
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let fixture = Fixture::new(config()).await;
                        harmos_runtime::storage::operations::register(
                            &fixture.runtime.operations,
                            "storage",
                            Lanes::Open,
                            Arc::new(fixture.storage.clone()),
                        )
                        .unwrap();
                        write(&fixture.storage, "historical").await;
                        fixture.storage.flush().await.unwrap();
                        let request = harmos_runtime::storage::operations::FollowRecords {
                            after: None,
                            selection: records(),
                        };
                        let limits = harmos_runtime::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        };
                        let (sender, mut receiver) =
                            harmos_runtime::operations::stream::<Followed>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowRecords, _>(request.clone(), sender)
                            .unwrap();
                        let Followed::Boundaries(boundaries) =
                            (*receiver.recv().await.unwrap()).clone()
                        else {
                            panic!("a follow first reports its boundaries");
                        };
                        assert_eq!(boundaries.saved, boundaries.accepted);
                        let first = followed(receiver.recv().await.unwrap());
                        assert_eq!(first.item.as_str(), "historical");
                        assert!(first.position.sequence <= boundaries.saved.sequence);
                        let second_position = write(&fixture.storage, "live").await;
                        let second = followed(receiver.recv().await.unwrap());
                        assert_eq!(second.item.as_str(), "live");
                        assert_eq!(second.position, second_position);
                        assert!(second.position.sequence > boundaries.accepted.sequence);
                        fixture.storage.flush().await.unwrap();
                        instance.cancel();
                        assert!(instance.await.is_err());
                        drop(receiver);
                        let mut request = request;
                        request.after = Some(second_position);
                        let (sender, mut receiver) =
                            harmos_runtime::operations::stream::<Followed>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowRecords, _>(request, sender)
                            .unwrap();
                        drop(receiver.recv().await.unwrap());
                        write(&fixture.storage, "reconnected").await;
                        let third = followed(receiver.recv().await.unwrap());
                        assert_eq!(third.item.as_str(), "reconnected");
                        instance.cancel();
                        assert!(instance.await.is_err());
                        fixture.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("follow_keeps_sequence", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos_runtime::storage::operations::FollowStream;
                    use harmos_runtime::storage::{StoredRow, StreamWrite};
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let fixture = Fixture::new(config()).await;
                        harmos_runtime::storage::operations::register(
                            &fixture.runtime.operations,
                            "storage",
                            Lanes::Open,
                            Arc::new(fixture.storage.clone()),
                        )
                        .unwrap();
                        let key = super::schema();
                        let stream = harmos_runtime::Identifier::new("voltage").unwrap();
                        let item = harmos_runtime::Identifier::new("row").unwrap();
                        let request = harmos_runtime::storage::operations::FollowStream {
                            after: None,
                            selection: harmos_runtime::storage::query::StreamSelection {
                                schema: key.clone(),
                                scope: None,
                                item: None,
                                stream: Some(stream.clone()),
                                source_start: None,
                                source_end: None,
                                after: 0,
                            },
                        };
                        let limits = harmos_runtime::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        };
                        let (sender, mut receiver) =
                            harmos_runtime::operations::stream::<StoredRow>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowStream, _>(request.clone(), sender)
                            .unwrap();
                        fixture
                            .storage
                            .write_stream(&[StreamWrite {
                                stream: &stream,
                                sequence: 17,
                                record: super::record(&key, &item, b"sample"),
                            }])
                            .await
                            .unwrap();
                        let row = receiver.recv().await.unwrap();
                        assert_eq!(row.sequence, 17);
                        assert_eq!(row.record.position.sequence, 1);
                        drop(row);
                        instance.cancel();
                        assert!(instance.await.is_err());
                        let mut foreign = fixture.storage.cursor_accepted();
                        foreign.history.epoch = harmos_runtime::Identifier::new("foreign").unwrap();
                        let mut request = request;
                        request.after = Some(foreign);
                        let (sender, mut receiver) =
                            harmos_runtime::operations::stream::<StoredRow>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowStream, _>(request, sender)
                            .unwrap();
                        assert!(instance.await.is_err());
                        assert!(receiver.recv().await.is_none());
                        fixture.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("following_spends_budget", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos_runtime::storage::operations::{FollowRecords, Followed};
                    use std::time::Duration;
                    let fixture = Fixture::new(config()).await;
                    harmos_runtime::storage::operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    write(&fixture.storage, "first").await;
                    let (sender, mut receiver) = harmos_runtime::operations::stream::<Followed>(
                        harmos_runtime::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        },
                    )
                    .unwrap();
                    let request = harmos_runtime::storage::operations::FollowRecords {
                        after: None,
                        selection: records(),
                    };
                    let instance = fixture
                        .runtime
                        .operations
                        .submit_endpoints::<FollowRecords, _>(request, sender)
                        .unwrap();
                    drop(receiver.recv().await.unwrap());
                    drop(receiver.recv().await.unwrap());
                    tokio::time::advance(Duration::from_secs(16)).await;
                    write(&fixture.storage, "after-attempt-cap").await;
                    let record = followed(receiver.recv().await.unwrap());
                    assert_eq!(record.item.as_str(), "after-attempt-cap");
                    tokio::time::advance(Duration::from_secs(44)).await;
                    assert!(instance.await.is_err());
                    assert!(receiver.recv().await.is_none());
                    fixture.finish().await;
                });
        });
    }
}

#[test]
fn maintenance_writes() {
    queries::contract_scenario();
    own_contract();
    deletion::contract_scenario();
}

#[test]
fn file_mutations() {
    files::contract_scenario();
    writes::contract_scenario();
}

#[test]
fn schema_registry() {
    #[cfg(feature = "plugin")]
    plugins::contract_scenario();
    schemas::contract_scenario();
}
