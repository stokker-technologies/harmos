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
use super::{Contract, ContractError, Contracts, Declarations, Definition, Entry, WireDeclaration};
use harmos_domain::Result;
use harmos_domain::catalog::{Catalog, Member, Visitor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct AuthoredDeclarations;

pub struct AuthoredContracts;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Visitor for AuthoredDeclarations {
    type Entry = Entry;
}

impl<T: super::Operation> Member<T> for AuthoredDeclarations {
    const ENTRY: Entry = Entry::of::<T::Declaration>();
}

impl AuthoredDeclarations {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Definition>> {
        Declarations::from_projection(&C::ENTRIES)
    }
}

impl Visitor for AuthoredContracts {
    type Entry = WireDeclaration;
}

impl<T: super::Operation> Member<T> for AuthoredContracts {
    const ENTRY: WireDeclaration = WireDeclaration::of::<T::Declaration>();
}

impl AuthoredContracts {
    pub fn of<C: Catalog<Self>>() -> std::result::Result<Vec<Contract>, ContractError> {
        Contracts::from_projection(&C::ENTRIES)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
