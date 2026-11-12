/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Register the Generated Host Catalog Operation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::List;

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

pub fn host_catalog() -> crate::execution::operations::Catalog {
    host_catalog_batch().named("harmos.host")
}

pub fn host_catalog_batch() -> crate::execution::__private::Catalog {
    crate::execution::__private::Catalog {
        name: "",
        operations: vec![crate::execution::operations::OperationFactory::protobuf::<List>().expect("catalog declaration")],
        services: vec![],
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
