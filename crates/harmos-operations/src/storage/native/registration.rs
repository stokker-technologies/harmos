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
        crate::execution::operations::OperationFactory::native::<super::CreateFileProduction>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetFileProduction>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::AppendFileBytes>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::CreateFileWriter>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::PublishFileProduction>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::DeleteFileProduction>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetFileBytes>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetIndexDefinition>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::SetIndexDefinition>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetProjectionDefinition>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::SetProjectionDefinition>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::ListRows>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::Capabilities>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::CreateSnapshot>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::DeleteSnapshot>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::ListColumnRows>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::ListIndexEntries>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::AdvanceProjection>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetProjectionStatus>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::ListProjectionRows>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetObject>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::SetObject>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::AppendRecords>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::AppendSamples>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::AppendObjectRevisions>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::WatchRows>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::WatchFileBytes>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::WatchFileProduction>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::WatchDirectoryEntries>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetFileHash>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::CaptureFile>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::ListDirectoryEntries>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::Info>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::DeleteFile>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::SetSchema>().expect("native storage declaration"),
        crate::execution::operations::OperationFactory::native::<super::GetSchema>().expect("native storage declaration"),
    ]);
    batch
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
