/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Transfer Operations for the Host and Guest.

/* === Modules === */

pub mod download;
pub mod files;
pub mod hash;
pub mod put;
pub mod store;
pub mod take;
pub mod upload;
pub mod uploads;

/* === Re-Exports === */

pub use download::Download;
pub use files::{FilesHash, FilesRead};
pub use hash::Hash;
pub use put::Put;
pub use store::Store;
pub use take::Take;
pub use upload::Upload;
pub use uploads::{UploadsAppend, UploadsCreate, UploadsDelete, UploadsGet, UploadsPublish, UploadsResume};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_operations::execution::operations::{Catalog, OperationFactory};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn transfers_catalog() -> Catalog {
    Catalog::new()
        .named("schematic.transfers")
        .entry(OperationFactory::protobuf::<Upload>().expect("Upload declaration"))
        .entry(OperationFactory::protobuf::<Download>().expect("Download declaration"))
        .entry(OperationFactory::protobuf::<Put>().expect("Put declaration"))
        .entry(OperationFactory::protobuf::<Take>().expect("Take declaration"))
        .entry(OperationFactory::protobuf::<FilesHash>().expect("FilesHash declaration"))
        .entry(OperationFactory::protobuf::<Hash>().expect("Hash declaration"))
}

pub fn files_catalog() -> Catalog {
    Catalog::new()
        .named("schematic.files")
        .entry(OperationFactory::protobuf::<FilesHash>().expect("FilesHash declaration"))
        .entry(OperationFactory::protobuf::<FilesRead>().expect("FilesRead declaration"))
        .entry(OperationFactory::protobuf::<UploadsCreate>().expect("UploadsCreate declaration"))
        .entry(OperationFactory::protobuf::<UploadsGet>().expect("UploadsGet declaration"))
        .entry(OperationFactory::protobuf::<UploadsResume>().expect("UploadsResume declaration"))
        .entry(OperationFactory::protobuf::<UploadsAppend>().expect("UploadsAppend declaration"))
        .entry(OperationFactory::protobuf::<UploadsPublish>().expect("UploadsPublish declaration"))
        .entry(OperationFactory::protobuf::<UploadsDelete>().expect("UploadsDelete declaration"))
}

/* === Private Functions === */

/* === Tests === */
