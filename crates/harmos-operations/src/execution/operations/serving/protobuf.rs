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
use std::any::Any;
// Third Party Imports

// Local Imports
use crate::execution::grpc::{self, DecodeInput};
#[cfg(feature = "streaming")]
use crate::execution::operations::Direction;
use crate::execution::{MessageError, Portable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Decode = fn(&[u8], &mut dyn Any) -> Result<(), MessageError>;
#[cfg(feature = "streaming")]
type Encode = fn(&dyn Any) -> Result<Vec<u8>, MessageError>;

#[cfg(feature = "streaming")]
type Prepare = fn(&[u8]) -> Result<(Vec<u8>, usize), MessageError>;

/* === Structs === */

#[derive(Clone, Copy)]
pub(crate) struct Codec {
    pub(crate) decode: Decode,

    #[cfg(feature = "streaming")]
    pub(super) input: Option<ItemCodec>,

    #[cfg(feature = "streaming")]
    pub(super) output: Option<ItemCodec>,
}

#[cfg(feature = "streaming")]
#[derive(Clone, Copy)]
pub(super) struct ItemCodec {
    pub(super) item_type: std::any::TypeId,
    pub(super) prepare: Prepare,
    pub(crate) decode: Decode,
    pub(crate) encode: Encode,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Codec {
    pub(crate) fn request<Q: crate::execution::operations::RequestShape>() -> Self {
        Self {
            decode: decode::<Q::Request>,
            #[cfg(feature = "streaming")]
            input: direction::<Q::Input>(),
            #[cfg(feature = "streaming")]
            output: direction::<Q::Output>(),
        }
    }

    // The One Protobuf Projection Of A Declaration: Request, Response And The
    // Item Codec Of Each Direction It Streams.
}

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "streaming")]
fn direction<D: Direction>() -> Option<ItemCodec> {
    D::STREAMED.then(item::<D::Item>)
}

#[cfg(feature = "streaming")]
fn item<T: Portable>() -> ItemCodec {
    ItemCodec {
        item_type: std::any::TypeId::of::<T>(),
        prepare: |payload| {
            let prepared = grpc::prepare::<T>(DecodeInput::new(payload))?;
            let memory = prepared.memory();
            Ok((prepared.into_postcard(), memory))
        },
        decode: decode_postcard::<T>,
        encode: encode::<T>,
    }
}

fn decode<T: Portable>(payload: &[u8], target: &mut dyn Any) -> Result<(), MessageError> {
    let target =
        target.downcast_mut::<Option<T>>().ok_or_else(|| MessageError::borrowed("protobuf request type differs from operation"))?;
    *target = Some(grpc::decode::<T>(DecodeInput::new(payload))?);
    Ok(())
}

#[cfg(feature = "streaming")]
#[tracing::instrument(target = "harmos::grpc", name = "grpc.postcard.decode", level = "trace", skip_all, fields(bytes = payload.len()))]
fn decode_postcard<T: Portable>(payload: &[u8], target: &mut dyn Any) -> Result<(), MessageError> {
    let target =
        target.downcast_mut::<Option<T>>().ok_or_else(|| MessageError::borrowed("protobuf stream item type differs from operation"))?;
    *target = Some(T::decode(payload)?);
    Ok(())
}

#[cfg(feature = "streaming")]
fn encode<T: Portable>(value: &dyn Any) -> Result<Vec<u8>, MessageError> {
    let value = value.downcast_ref::<T>().ok_or_else(|| MessageError::borrowed("protobuf response type differs from operation"))?;
    let mut bytes = Vec::new();
    grpc::encode(value, &mut bytes)?;
    Ok(bytes)
}

/* === Tests === */
