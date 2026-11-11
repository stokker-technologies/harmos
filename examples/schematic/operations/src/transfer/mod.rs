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
pub mod hash;
pub mod put;
#[cfg(feature = "runtime")]
pub mod store;
pub mod take;
pub mod upload;

/* === Re-Exports === */

pub use download::Download;
pub use hash::Hash;
pub use put::Put;
#[cfg(feature = "runtime")]
pub use store::Store;
pub use take::Take;
pub use upload::Upload;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub struct Transfers;
impl harmos_operations::execution::operations::OperationCatalog for Transfers {
    const CATALOG: harmos_operations::execution::operations::Catalog =
        harmos_operations::execution::operations::Catalog::of::<Self>("schematic.transfers", &[], &[]);
    fn batch() -> harmos_operations::execution::__private::RegistrationBatch {
        let mut batch = harmos_operations::execution::__private::RegistrationBatch::new();
        batch.operations.extend([
            harmos_operations::execution::operations::OperationFactory::protobuf::<Upload>().expect("Upload declaration"),
            harmos_operations::execution::operations::OperationFactory::protobuf::<Download>().expect("Download declaration"),
            harmos_operations::execution::operations::OperationFactory::protobuf::<Put>().expect("Put declaration"),
            harmos_operations::execution::operations::OperationFactory::protobuf::<Take>().expect("Take declaration"),
            harmos_operations::execution::operations::OperationFactory::protobuf::<Hash>().expect("Hash declaration"),
        ]);
        batch
    }
}
