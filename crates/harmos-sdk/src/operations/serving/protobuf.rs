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
use crate::grpc::{self, DecodeInput};
#[cfg(feature = "streaming")]
use crate::operations::Direction;
use crate::operations::OperationDeclaration;
use crate::{MessageError, Portable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Decode = fn(&[u8], &mut dyn Any) -> Result<(), MessageError>;
type Encode = fn(&dyn Any) -> Result<Vec<u8>, MessageError>;

/* === Structs === */
#[derive(Clone, Copy)]
pub(super) struct Codec {
    pub(super) decode: Decode,
    pub(super) encode: Encode,
    #[cfg(feature = "streaming")]
    pub(super) input: Option<ItemCodec>,
    #[cfg(feature = "streaming")]
    pub(super) output: Option<ItemCodec>,
}

#[cfg(feature = "streaming")]
#[derive(Clone, Copy)]
pub(super) struct ItemCodec {
    pub(super) item_type: std::any::TypeId,
    pub(super) memory: fn(&[u8]) -> Result<usize, MessageError>,
    pub(super) decode: Decode,
    pub(super) length: fn(&dyn Any) -> Result<usize, MessageError>,
    pub(super) encode: Encode,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Codec {
    // The One Protobuf Projection Of A Declaration: Request, Response And The
    // Item Codec Of Each Direction It Streams.
    pub(crate) fn of<O: OperationDeclaration>() -> Self {
        Self {
            decode: decode::<O>,
            encode: encode::<O::Response>,
            #[cfg(feature = "streaming")]
            input: direction::<O::Input>(),
            #[cfg(feature = "streaming")]
            output: direction::<O::Output>(),
        }
    }
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
        memory: |payload| grpc::decoding_memory::<T>(DecodeInput::new(payload)),
        decode: decode::<T>,
        length: |value| {
            value.downcast_ref::<T>().ok_or_else(|| MessageError::borrowed("protobuf stream item type differs from operation")).and_then(
                |value| {
                    let mut bytes = Vec::new();
                    grpc::encode(value, &mut bytes)?;
                    Ok(bytes.len())
                },
            )
        },
        encode: encode::<T>,
    }
}

fn decode<T: Portable>(payload: &[u8], target: &mut dyn Any) -> Result<(), MessageError> {
    let target =
        target.downcast_mut::<Option<T>>().ok_or_else(|| MessageError::borrowed("protobuf request type differs from operation"))?;
    grpc::decoding_memory::<T>(DecodeInput::new(payload))?;
    *target = Some(grpc::decode::<T>(DecodeInput::new(payload))?);
    Ok(())
}

fn encode<T: Portable>(value: &dyn Any) -> Result<Vec<u8>, MessageError> {
    let value = value.downcast_ref::<T>().ok_or_else(|| MessageError::borrowed("protobuf response type differs from operation"))?;
    let mut bytes = Vec::new();
    grpc::encode(value, &mut bytes)?;
    Ok(bytes)
}

/* === Tests === */
