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

use super::Catalog;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Host;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Host {
    pub const CATALOG: crate::execution::operations::Catalog = <Self as crate::execution::operations::OperationCatalog>::CATALOG;
    pub fn catalog() -> Vec<crate::OperationDefinition> {
        <Self as crate::execution::operations::OperationCatalog>::batch().operations.into_iter().map(|factory| factory.definition).collect()
    }
}
impl crate::execution::operations::OperationCatalog for Host {
    const CATALOG: crate::execution::operations::Catalog = crate::execution::operations::Catalog::of::<Self>("harmos.host", &[], &[]);
    fn batch() -> crate::execution::__private::RegistrationBatch {
        crate::execution::__private::RegistrationBatch {
            operations: vec![crate::execution::operations::OperationFactory::protobuf::<Catalog>().expect("catalog declaration")],
            services: vec![],
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
