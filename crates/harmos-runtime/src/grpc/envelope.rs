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
use super::codec::Wire;
use crate::{MemorySize, MessageError};
use harmos_domain::__private::decoding_add;
use harmos_sdk::grpc::{DecodeInput, prost};

/* === Constants === */

/* === Macros === */

macro_rules! envelope {
    ($name:ident, $first:ident, $second:ident) => {
        impl<A: MemorySize, B: MemorySize> $name<A, B> {
            pub fn project(
                item: crate::operations::StreamItem<Self>,
            ) -> crate::Result<$name<crate::operations::StreamItem<A>, crate::operations::StreamItem<B>>> {
                Ok(
                    match item.project(|value| match value {
                        Self::$first(value) => Ok(value),
                        Self::$second(value) => Err(value),
                    })? {
                        Ok(value) => $name::$first(value),
                        Err(value) => $name::$second(value),
                    },
                )
            }
        }
        impl<W, A, B> Wire<$name<A, B>> for W
        where
            W: Wire<A> + Wire<B>,
            A: Send + Sync + 'static,
            B: Send + Sync + 'static,
        {
            fn encode(value: &$name<A, B>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
                match value {
                    $name::$first(value) => delimited(1, bytes, |bytes| <W as Wire<A>>::encode(value, bytes)),
                    $name::$second(value) => delimited(2, bytes, |bytes| <W as Wire<B>>::encode(value, bytes)),
                }
            }
            fn decode_admitted<C>(
                input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, tonic::Status>,
            ) -> Result<($name<A, B>, C), tonic::Status> {
                let (tag, payload) = input
                    .variant(&[1, 2])
                    .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?
                    .ok_or_else(|| tonic::Status::invalid_argument("empty operation envelope"))?;
                let admit = |bound| {
                    let bytes = decoding_add(std::mem::size_of::<$name<A, B>>(), bound)
                        .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?;
                    admit(bytes)
                };
                match tag {
                    1 => <W as Wire<A>>::decode_admitted(payload, admit).map(|(value, claim)| ($name::$first(value), claim)),
                    _ => <W as Wire<B>>::decode_admitted(payload, admit).map(|(value, claim)| ($name::$second(value), claim)),
                }
            }
        }
        impl<A: MemorySize, B: MemorySize> MemorySize for $name<A, B> {
            fn memory_size(&self) -> usize {
                let heap = match self {
                    Self::$first(value) => value.memory_size().saturating_sub(std::mem::size_of::<A>()),
                    Self::$second(value) => value.memory_size().saturating_sub(std::mem::size_of::<B>()),
                };
                std::mem::size_of::<Self>().saturating_add(heap)
            }
        }
    };
}

envelope!(InputEnvelope, Request, Item);

envelope!(OutputEnvelope, Item, Response);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, PartialEq)]
pub enum InputEnvelope<R, I> {
    Request(R),
    Item(I),
}

#[derive(Debug, PartialEq)]
pub enum OutputEnvelope<I, R> {
    Item(I),
    Response(R),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Frame One Length-Delimited Field Around A Payload Written Into Scratch.
pub(super) fn delimited(
    tag: u32, bytes: &mut Vec<u8>, write: impl FnOnce(&mut Vec<u8>) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    let mut payload = Vec::new();
    write(&mut payload)?;
    prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, bytes);
    prost::encoding::encode_varint(payload.len() as u64, bytes);
    bytes.extend_from_slice(&payload);
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
