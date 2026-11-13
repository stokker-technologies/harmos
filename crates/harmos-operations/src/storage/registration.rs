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

use super::{DeleteFile, Info, ReadFile, SetSchema, WriteFile};

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
        crate::execution::operations::OperationFactory::protobuf::<super::GetFileHash>().expect("GetFileHash declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetFileHash>().expect("GetFileHash declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::CaptureFile>().expect("CaptureFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::CaptureFile>().expect("CaptureFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ListDirectoryEntries>()
            .expect("ListDirectoryEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::ListDirectoryEntries>()
            .expect("ListDirectoryEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::WatchFileBytes>().expect("WatchFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::WatchFileBytes>().expect("WatchFileBytes declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::WatchFileProduction>().expect("WatchFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::WatchFileProduction>()
            .expect("WatchFileProduction declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::WatchDirectoryEntries>()
            .expect("WatchDirectoryEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::WatchDirectoryEntries>()
            .expect("WatchDirectoryEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::WatchRows>().expect("WatchRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::WatchRows>().expect("WatchRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AppendRecords>().expect("AppendRecords declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AppendSamples>().expect("AppendSamples declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AppendObjectRevisions>()
            .expect("AppendObjectRevisions declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::AppendRecords>().expect("AppendRecords declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::AppendSamples>().expect("AppendSamples declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::AppendObjectRevisions>()
            .expect("AppendObjectRevisions declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::ListProjectionRows>()
            .expect("ListProjectionRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ListProjectionRows>().expect("ListProjectionRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetProjectionStatus>()
            .expect("GetProjectionStatus declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetProjectionStatus>().expect("GetProjectionStatus declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::AdvanceProjection>()
            .expect("AdvanceProjection declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::AdvanceProjection>().expect("AdvanceProjection declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::ListIndexEntries>()
            .expect("ListIndexEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ListIndexEntries>().expect("ListIndexEntries declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::ListColumnRows>().expect("ListColumnRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ListColumnRows>().expect("ListColumnRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::DeleteSnapshot>().expect("DeleteSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::DeleteSnapshot>().expect("DeleteSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::CreateSnapshot>().expect("CreateSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::CreateSnapshot>().expect("CreateSnapshot declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::Capabilities>().expect("Capabilities declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::Capabilities>().expect("Capabilities declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::ListRows>().expect("ListRows declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::ListRows>().expect("ListRows declaration"),
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
        crate::execution::operations::OperationFactory::protobuf::<super::GetObject>().expect("GetObject declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::SetObject>().expect("SetObject declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::GetObject>().expect("GetObject declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::context::SetObject>().expect("SetObject declaration"),
        crate::execution::operations::OperationFactory::protobuf::<WriteFile>().expect("WriteFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<SetSchema>().expect("SetSchema declaration"),
        crate::execution::operations::OperationFactory::protobuf::<ReadFile>().expect("ReadFile declaration"),
        crate::execution::operations::OperationFactory::protobuf::<super::GetSchema>().expect("GetSchema declaration"),
        crate::execution::operations::OperationFactory::protobuf::<Info>().expect("Info declaration"),
        crate::execution::operations::OperationFactory::protobuf::<DeleteFile>().expect("DeleteFile declaration"),
    ]);
    batch
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
