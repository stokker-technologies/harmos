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

pub use payloads::{FollowingRow, PublishRequest};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

// A Sidecar Publishing One Ordered Batch Of Stream Rows To Its Host.
pub struct Publish;

impl crate::operations::OperationDeclaration for Publish {
    type Request = PublishRequest;
    type Response = ();
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "host_publish";
    const POLICY: crate::operations::OperationPolicy = crate::operations::lifecycle::POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
}

impl crate::operations::RequestFor for PublishRequest {
    type Operation = Publish;
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
