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
#[path = "saves.rs"]
mod saves;
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
use harmos::operations::Contracts;
use harmos::storage::operations::{Operations, Save, SaveRequest, Selection};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::lanes]
pub(super) enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn followed(
    item: harmos::operations::StreamItem<harmos::storage::operations::Followed>,
) -> harmos::storage::StoredRecord {
    match &*item {
        harmos::storage::operations::Followed::Record(record) => record.clone(),
        harmos::storage::operations::Followed::Boundaries(_) => {
            panic!("boundaries are reported once, before records")
        }
    }
}

pub(super) fn save_request(
    fixture: &Fixture,
    through: harmos::storage::StorageCursor,
) -> SaveRequest {
    SaveRequest {
        mutation: harmos::storage::MutationId {
            window: fixture.storage.mutation_window().unwrap().id,
            key: harmos::Identifier::new("save").unwrap(),
        },
        through,
    }
}

/* === Tests === */

fn own_contract() {
    {
        scenario_runtime::check("operation_preserves_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    Operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    write(&fixture.storage, "accepted").await;
                    let through = fixture.storage.cursor_accepted();
                    assert!(fixture.storage.cursor_saved().sequence < through.sequence);
                    let saved = fixture
                        .runtime
                        .operations
                        .call_as::<Save>(save_request(&fixture, through.clone()))
                        .await
                        .unwrap();
                    assert!(saved.covers(&through).unwrap());
                    assert_eq!(*saved, fixture.storage.cursor_saved());
                    let repeated = fixture
                        .runtime
                        .operations
                        .call_as::<Save>(save_request(&fixture, through.clone()))
                        .await
                        .unwrap();
                    assert_eq!(*repeated, *saved);
                    assert_eq!(fixture.storage.cursor_accepted(), through);
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("operation_rejects_future", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    Operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    let mut foreign = fixture.storage.cursor_accepted();
                    foreign.history.epoch = harmos::Identifier::new("different").unwrap();
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .call_as::<Save>(save_request(&fixture, foreign))
                            .await
                            .is_err()
                    );
                    let mut future = fixture.storage.cursor_accepted();
                    future.sequence += 1;
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .call_as::<Save>(save_request(&fixture, future))
                            .await
                            .is_err()
                    );
                    assert_eq!(fixture.storage.cursor_saved().sequence, 0);
                    fixture.finish().await;
                });
        });
    }
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
            let artifacts = harmos::grpc::Artifacts::new(&contracts).unwrap();
            let target = tempfile::tempdir().unwrap();
            let root = artifacts.export(target.path(), "storage").unwrap();
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
        scenario_runtime::check("save_retries_three", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(config()).await;
                    Operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    let through = write(&fixture.storage, "retry").await;
                    fixture
                        .data
                        .control
                        .failures
                        .store(3, std::sync::atomic::Ordering::SeqCst);
                    let saved = fixture
                        .runtime
                        .operations
                        .call_as::<Save>(save_request(&fixture, through.clone()))
                        .await
                        .unwrap();
                    assert_eq!(*saved, through);
                    assert_eq!(fixture.storage.cursor_accepted(), through);
                    assert_eq!(
                        fixture
                            .data
                            .control
                            .failures
                            .load(std::sync::atomic::Ordering::SeqCst),
                        0
                    );
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("save_retry_budget", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_retries = 3;
                    let fixture = Fixture::new(settings).await;
                    Operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    let through = write(&fixture.storage, "retry-budget").await;
                    fixture
                        .data
                        .control
                        .failures
                        .store(5, std::sync::atomic::Ordering::SeqCst);
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .call_as::<Save>(save_request(&fixture, through.clone()))
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        fixture
                            .data
                            .control
                            .failures
                            .load(std::sync::atomic::Ordering::SeqCst),
                        1
                    );
                    assert_eq!(fixture.storage.cursor_saved().sequence, 0);
                    fixture
                        .data
                        .control
                        .failures
                        .store(0, std::sync::atomic::Ordering::SeqCst);
                    fixture
                        .runtime
                        .operations
                        .call_as::<Save>(save_request(&fixture, through))
                        .await
                        .unwrap();
                    fixture.finish().await;
                });
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
                    use harmos::storage::operations::{DataSelection, Follow, Followed};
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let fixture = Fixture::new(config()).await;
                        Operations::register(
                            &fixture.runtime.operations,
                            "storage",
                            Lanes::Open,
                            Arc::new(fixture.storage.clone()),
                        )
                        .unwrap();
                        write(&fixture.storage, "historical").await;
                        fixture.storage.flush().await.unwrap();
                        let request = DataSelection {
                            selection: Selection {
                                page_rows: 1,
                                page_bytes: 4096,
                                ..Selection::default()
                            },
                            schema: super::schema(),
                            stream: None,
                            source_start: None,
                            source_end: None,
                            record_filter: None,
                        };
                        let limits = harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        };
                        let (sender, mut receiver) =
                            harmos::operations::stream::<Followed>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<Follow, _>(request.clone(), sender)
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
                        request.selection.after = Some(second_position);
                        let (sender, mut receiver) =
                            harmos::operations::stream::<Followed>(limits).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<Follow, _>(request, sender)
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
                    use harmos::storage::operations::{DataSelection, FollowStream};
                    use harmos::storage::{StoredRow, StreamWrite};
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let fixture = Fixture::new(config()).await;
                        Operations::register(
                            &fixture.runtime.operations,
                            "storage",
                            Lanes::Open,
                            Arc::new(fixture.storage.clone()),
                        )
                        .unwrap();
                        let key = super::schema();
                        let stream = harmos::Identifier::new("voltage").unwrap();
                        let item = harmos::Identifier::new("row").unwrap();
                        let request = DataSelection {
                            selection: Selection::default(),
                            schema: key.clone(),
                            stream: Some(stream.clone()),
                            source_start: None,
                            source_end: None,
                            record_filter: None,
                        };
                        let limits = harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        };
                        let (sender, mut receiver) =
                            harmos::operations::stream::<StoredRow>(limits).unwrap();
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
                        foreign.history.epoch = harmos::Identifier::new("foreign").unwrap();
                        let mut request = request;
                        request.selection.after = Some(foreign);
                        let (sender, mut receiver) =
                            harmos::operations::stream::<StoredRow>(limits).unwrap();
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
                    use harmos::storage::operations::{DataSelection, Follow, Followed};
                    use std::time::Duration;
                    let fixture = Fixture::new(config()).await;
                    Operations::register(
                        &fixture.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(fixture.storage.clone()),
                    )
                    .unwrap();
                    write(&fixture.storage, "first").await;
                    let (sender, mut receiver) = harmos::operations::stream::<Followed>(
                        harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        },
                    )
                    .unwrap();
                    let request = DataSelection {
                        selection: Selection::default(),
                        schema: super::schema(),
                        stream: None,
                        source_start: None,
                        source_end: None,
                        record_filter: None,
                    };
                    let instance = fixture
                        .runtime
                        .operations
                        .submit_endpoints::<Follow, _>(request, sender)
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
    saves::contract_scenario();
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
