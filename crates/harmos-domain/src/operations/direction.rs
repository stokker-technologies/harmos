/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Stream Direction Of An Operation: `Stream<T>` Carries Items Of `T`;
// `NoStream` Is The Uninhabited Item Of A Direction The Operation Does Not Use.

use std::any::TypeId;
use std::marker::PhantomData;

use crate::schema::Described;
use crate::schema::declaration::Definition;
use crate::schema::graph::Field;
use crate::{MemorySize, MessageError};

pub struct Stream<T>(PhantomData<fn() -> T>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoStream {}

pub trait Direction: Send + Sync + 'static {
    type Item: crate::Portable;
    const STREAMED: bool;

    fn schema() -> Option<Definition> {
        Self::STREAMED.then_some(<Self::Item as Described>::DEFINITION)
    }

    fn item_type() -> Option<TypeId> {
        Self::STREAMED.then(TypeId::of::<Self::Item>)
    }
}

impl<T: crate::Portable> Direction for Stream<T> {
    type Item = T;
    const STREAMED: bool = true;
}

impl Direction for NoStream {
    type Item = NoStream;
    const STREAMED: bool = false;
}

impl Described for NoStream {
    const NAME: &'static str = "harmos::no_stream";
    const FIELDS: &'static [Field<Definition>] = &[];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl MemorySize for NoStream {
    fn memory_size(&self) -> usize {
        match *self {}
    }
}

impl crate::__private::PostcardValue for NoStream {
    fn write(&self, _: &mut Vec<u8>) {
        match *self {}
    }
    fn size(&self) -> usize {
        match *self {}
    }
    fn memory(_: &mut crate::__private::PostcardReader<'_>) -> Result<usize, MessageError> {
        Err(absent())
    }
    fn read(_: &mut crate::__private::PostcardReader<'_>) -> Result<Self, MessageError> {
        Err(absent())
    }
}

// The Item Types An Operation Streams, Input Then Output.
pub fn item_types<O: super::OperationDeclaration>() -> (Option<TypeId>, Option<TypeId>) {
    (<O::Input as Direction>::item_type(), <O::Output as Direction>::item_type())
}

fn absent() -> MessageError {
    MessageError::borrowed("the operation streams no items in this direction")
}
