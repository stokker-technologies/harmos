/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::Contract;
use crate::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// A Live Provider Contract. Invocation Still Performs Request-Specific Authorization.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredOperation {
    pub owner: crate::Identifier,
    pub generation: u64,
    pub contract: Contract,
    pub grants: Vec<Grant>,
    pub conditional_access: bool,
    pub grpc_available: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
