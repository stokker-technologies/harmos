/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive a File Through Generated Mutation Operations.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn create_append_resume_finish_and_delete() {
    use harmos_operations::storage::{AbandonFile, AppendFile, CreateFile, DeleteFile, FinishFile, ResumeFile};
    use harmos_protocol::storage as wire;
    let fixture = super::Fixture::new(super::config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(super::support::Lanes::Open).await.unwrap();
    let operations = &fixture.runtime.operations;
    let window = fixture.storage.mutation_window().unwrap().id.to_string();
    let mutation = |key: &str| Some(wire::MutationId { window: window.clone(), key: key.into() });
    let create = wire::CreateFile {
        mutation: mutation("create"),
        metadata: Some(wire::FileMetadata {
            id: "generated-file".into(),
            media_type: "application/octet-stream".into(),
            ..Default::default()
        }),
    };
    let opened = operations.call_protobuf::<CreateFile>(create.clone()).await.unwrap();
    assert_eq!(operations.call_protobuf::<CreateFile>(create).await.unwrap(), opened);
    let append = wire::AppendFile {
        mutation: mutation("append"),
        session: opened.session,
        offset: 0,
        bytes: b"protobuf-file".to_vec(),
    };
    let appended = operations.call_protobuf::<AppendFile>(append.clone()).await.unwrap();
    assert_eq!(appended.length, 13);
    assert_eq!(operations.call_protobuf::<AppendFile>(append).await.unwrap(), appended);
    let resumed = operations
        .call_protobuf::<ResumeFile>(wire::ResumeFile {
            mutation: mutation("resume"),
            session: appended.session.clone(),
        })
        .await
        .unwrap();
    assert!(resumed.session.as_ref().unwrap().generation > appended.session.as_ref().unwrap().generation);
    let reference = operations
        .call_protobuf::<FinishFile>(wire::FinishFile {
            mutation: mutation("finish"),
            session: resumed.session,
        })
        .await
        .unwrap();
    assert_eq!(reference.length, 13);
    let file = wire::StagedFile {
        history: reference.position.unwrap().history,
        id: reference.metadata.unwrap().id,
    };
    let deleted = operations
        .call_protobuf::<DeleteFile>(wire::DeleteFile {
            mutation: mutation("delete"),
            file: Some(file.clone()),
        })
        .await
        .unwrap();
    assert_eq!(deleted, file);
    let staged = harmos_runtime::storage::StagedFile::try_from(file).unwrap();
    assert!(fixture.storage.follow_production(&staged, 0).await.is_err());
    let opened = operations
        .call_protobuf::<CreateFile>(wire::CreateFile {
            mutation: mutation("create-abandoned"),
            metadata: Some(wire::FileMetadata {
                id: "abandoned-file".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let abandoned = operations
        .call_protobuf::<AbandonFile>(wire::AbandonFile {
            mutation: mutation("abandon"),
            session: opened.session,
        })
        .await
        .unwrap();
    assert_eq!(abandoned.phase, wire::FilePhase::Abandoned as i32);
    fixture.finish().await;
}
