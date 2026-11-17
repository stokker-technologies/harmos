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

use super::{Get, List, Schemas, Streams};

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
        operations: vec![
            crate::execution::operations::OperationFactory::protobuf::<List>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::host::List>)
                .expect("operations declaration"),
            crate::execution::operations::OperationFactory::protobuf::<Get>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::host::Get>)
                .expect("operation declaration"),
            crate::execution::operations::OperationFactory::protobuf::<Schemas>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::host::Schemas>)
                .expect("schemas declaration"),
            crate::execution::operations::OperationFactory::protobuf::<Streams>()
                .and_then(crate::execution::operations::OperationFactory::with_native::<crate::host::declarations::host::Streams>)
                .expect("streams declaration"),
        ],
        services: vec![],
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
