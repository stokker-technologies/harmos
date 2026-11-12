/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Register the Storage Operation Families.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{
    CaptureFile, DeleteFile, FollowFile, FollowObjects, FollowProduction, FollowRecords, FollowStream, HashFile, ListDirectory, Query,
    ReadFile, StorageStatus, WatchDirectory, WriteFile, WriteObjects, WriteRecords, WriteSchema, WriteStreams,
};

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

pub fn storage_catalog() -> crate::execution::operations::Catalog {
    storage_catalog_batch().named("harmos.storage")
}

pub fn storage_catalog_batch() -> crate::execution::__private::Catalog {
    let mut batch = crate::execution::__private::Catalog::new();
    batch.operations.extend([
        crate::execution::operations::OperationFactory::protobuf::<super::context::SetProjectionDefinition>()
            .expect("SetProjectionDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::SetProjectionDefinition>()
            .expect("SetProjectionDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetProjectionDefinition>()
            .expect("GetProjectionDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetProjectionDefinition>()
            .expect("GetProjectionDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::SetIndexDefinition>()
            .expect("SetIndexDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::SetIndexDefinition>().expect("SetIndexDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetIndexDefinition>()
            .expect("GetIndexDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetIndexDefinition>().expect("GetIndexDefinition declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::CreateFileProduction>()
            .expect("CreateFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetFileProduction>().expect("GetFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AppendFileBytes>().expect("AppendFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::CreateFileWriter>().expect("CreateFileWriter declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::PublishFileProduction>()
            .expect("PublishFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::DeleteFileProduction>()
            .expect("DeleteFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetFileBytes>().expect("GetFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::CreateFileProduction>()
            .expect("CreateFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetFileProduction>()
            .expect("GetFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::AppendFileBytes>().expect("AppendFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::CreateFileWriter>()
            .expect("CreateFileWriter declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::PublishFileProduction>()
            .expect("PublishFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::DeleteFileProduction>()
            .expect("DeleteFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetFileBytes>().expect("GetFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::Capabilities>().expect("Capabilities declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::OpenSnapshot>().expect("OpenSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ReleaseSnapshot>().expect("ReleaseSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ScanArrow>().expect("ScanArrow declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ScanIndex>().expect("ScanIndex declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AdvanceProjection>().expect("AdvanceProjection declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ReadProjection>().expect("ReadProjection declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::InspectProjection>().expect("InspectProjection declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::CompareExchangeObject>()
            .expect("CompareExchangeObject declaration"),
        crate::execution::operations::OperationFactory::protobuf::<HashFile>().expect("HashFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteFile>().expect("WriteFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteRecords>().expect("WriteRecords declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteStreams>().expect("WriteStreams declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteObjects>().expect("WriteObjects declaration"),
        crate::execution::operations::OperationFactory::protobuf::<FollowRecords>().expect("FollowRecords declaration"),
        crate::execution::operations::OperationFactory::protobuf::<FollowStream>().expect("FollowStream declaration"),
        crate::execution::operations::OperationFactory::protobuf::<FollowObjects>().expect("FollowObjects declaration"),
        crate::execution::operations::OperationFactory::protobuf::<Query>().expect("Query declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteSchema>().expect("WriteSchema declaration"),
        crate::execution::operations::OperationFactory::protobuf::<ReadFile>().expect("ReadFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<CaptureFile>().expect("CaptureFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<FollowFile>().expect("FollowFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<FollowProduction>().expect("FollowProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<StorageStatus>().expect("StorageStatus declaration"),
        crate::execution::operations::OperationFactory::protobuf::<DeleteFile>().expect("DeleteFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<ListDirectory>().expect("ListDirectory declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WatchDirectory>().expect("WatchDirectory declaration"),
    ]);
    batch
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
