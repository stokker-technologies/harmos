/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod payloads;

/* === Re-Exports === */

pub use payloads::{FollowingRow, Publish};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

// A Sidecar Publishing One Ordered Batch Of Stream Rows To Its Host.
crate::declare!(Publish {
    id: "host.publish",
    version: 1,
    policy: crate::operations::host::POLICY,
    response: (),
    visibility: Visibility::Shared,
});

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
