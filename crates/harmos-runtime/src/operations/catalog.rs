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
use std::marker::PhantomData;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{Bindings, Context, Operation};
use crate::catalog::{Member, Visitor};
use crate::schema::Described;
use crate::{Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Register<H> = fn(&mut Bindings<'_>, &'static str, Arc<H>) -> Result<()>;

/* === Structs === */

pub struct Implementations<R>(PhantomData<fn() -> R>);

pub struct WireImplementations<R>(PhantomData<fn() -> R>);

pub struct Binding<H> {
    pub(super) register: Register<H>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<R: Send + Sync + 'static> Visitor for Implementations<R> {
    type Entry = Binding<R>;
}

impl<R: Send + Sync + 'static> Visitor for WireImplementations<R> {
    type Entry = Binding<R>;
}

impl<T, R> Member<T> for Implementations<R>
where
    R: Send + Sync + 'static,
    T: for<'ctx> Operation<Resources = R, Context<'ctx> = Context<'ctx>>,
{
    const ENTRY: Binding<R> = Binding { register: register_owned::<T> };
}

impl<T, R> Member<T> for WireImplementations<R>
where
    R: Send + Sync + 'static,
    T: for<'ctx> Operation<Resources = R, Context<'ctx> = Context<'ctx>>,
    T::Declaration: crate::operations::OperationDeclaration,
    T::Declaration: Message + Described,
{
    const ENTRY: Binding<R> = Binding { register: register_owned_wire::<T> };
}

impl<H> Binding<H> {
    pub(super) fn apply(&self, bindings: &mut Bindings<'_>, lane: &'static str, handler: Arc<H>) -> Result<()> {
        (self.register)(bindings, lane, handler)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn register_owned<T>(bindings: &mut Bindings<'_>, lane: &'static str, resources: Arc<T::Resources>) -> Result<()>
where
    T: for<'ctx> Operation<Context<'ctx> = Context<'ctx>>,
{
    bindings.register_owned_at::<T>(lane, resources)
}

fn register_owned_wire<T>(bindings: &mut Bindings<'_>, lane: &'static str, resources: Arc<T::Resources>) -> Result<()>
where
    T: for<'ctx> Operation<Context<'ctx> = Context<'ctx>>,
    T::Declaration: crate::operations::OperationDeclaration,
    T::Declaration: Message + Described,
{
    bindings.register_owned_wire_at::<T>(lane, resources)
}

/* === Tests === */
