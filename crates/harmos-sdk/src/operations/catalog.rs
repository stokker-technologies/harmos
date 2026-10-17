/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use harmos_domain::catalog::{Catalog, Member, Visitor};
use super::{Contract, ContractError, Contracts, Declarations, Definition, Entry, WireDeclaration};
use harmos_domain::Result;

/* === Structs === */
pub struct AuthoredDeclarations;
pub struct AuthoredContracts;

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
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Contract>, ContractError> {
        Contracts::from_projection(&C::ENTRIES)
    }
}

