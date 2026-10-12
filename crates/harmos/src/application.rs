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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub trait Application: Send + Sync + 'static {
    type State: Send + Sync + 'static;

    type Transaction: crate::Transaction<Self::State, Error: Send + Sync + 'static>
        + crate::TransactionCatalog<Self::State>
        + Send
        + Sync
        + 'static;

    type Record: crate::RecordCatalog + Send + Sync + 'static;

    type Streams: Send + Sync + 'static;

    type Resources: Send + Sync + 'static;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
