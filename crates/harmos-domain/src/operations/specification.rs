/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Operation Values

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Direction, OperationDefinition, WireDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct OperationSpec<Q: RequestShape> {
    pub declaration: &'static WireDeclaration,
    pub callees: &'static [WireDeclaration],
    pub grant: fn(&Q::Request) -> Option<crate::plugin::Grant>,
    marker: std::marker::PhantomData<fn() -> Q>,
}

/* === Enums === */

/* === Traits === */

pub trait RequestShape: 'static {
    type Request: crate::Portable;
    type Response: crate::Portable;
    type Input: Direction;
    type Output: Direction;

    fn fingerprint(id: &str) -> crate::Result<crate::Fingerprint>
    where
        Self: Sized,
    {
        super::catalog::request_fingerprint::<Self>(id)
    }
}

/* === Implementations === */

impl<Q: RequestShape> Copy for OperationSpec<Q> {}

impl<Q: RequestShape> Clone for OperationSpec<Q> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Q: RequestShape> OperationSpec<Q> {
    pub const fn new(declaration: &'static WireDeclaration) -> Self {
        Self {
            declaration,
            callees: &[],
            grant: |_| None,
            marker: std::marker::PhantomData,
        }
    }

    pub const fn calls(mut self, callees: &'static [WireDeclaration]) -> Self {
        self.callees = callees;
        self
    }

    pub const fn grant(mut self, grant: fn(&Q::Request) -> Option<crate::plugin::Grant>) -> Self {
        self.grant = grant;
        self
    }

    pub fn definition(&self) -> crate::Result<OperationDefinition> {
        let definition = self.declared()?;
        definition.validate()?;
        Ok(definition)
    }

    pub const fn wire(self) -> WireDeclaration {
        self.declaration.copied()
    }

    pub const fn entry(self) -> super::Entry {
        self.declaration.operation
    }

    pub const fn callees(self) -> &'static [WireDeclaration] {
        self.callees
    }

    #[doc(hidden)]
    pub fn declared(&self) -> crate::Result<OperationDefinition> {
        let definition = self.declaration.operation.declared()?;
        if definition.fingerprint != Q::fingerprint(self.declaration.operation.id)? {
            return Err(crate::MessageError::borrowed("operation description differs from its request shape").into());
        }
        Ok(definition)
    }

    pub fn named(&self, id: &str) -> crate::Result<OperationDefinition> {
        let mut definition = self.definition()?;
        definition.id = crate::Identifier::new(id)?;
        definition.fingerprint = (self.declaration.operation.fingerprint)(id)?;
        definition.validate()?;
        Ok(definition)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
