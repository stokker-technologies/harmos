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

#[harmos::catalog]
pub enum Transfers {
    Upload(Upload),
    Download(Download),
    Put(Put),
    Take(Take),
    Hash(Hash),
}

#[harmos::catalog]
pub enum Guest {
    #[nested]
    Shared(crate::Operations),

    #[nested]
    Transfers(Transfers),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
