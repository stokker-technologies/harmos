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
pub mod store;
pub mod take;
pub mod upload;

/* === Re-Exports === */

pub use download::Download;
pub use hash::Hash;
pub use put::Put;
pub use store::Store;
pub use take::Take;
pub use upload::Upload;

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
        .entry(OperationFactory::protobuf::<Hash>().expect("Hash declaration"))
}

/* === Private Functions === */

/* === Tests === */
