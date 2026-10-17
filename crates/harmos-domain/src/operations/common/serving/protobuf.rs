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
use crate::operations::{GrpcVisitor, InputDeclaration, OutputDeclaration, WireOperation};
use crate::{MemorySize, MessageError, Protobuf};

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
pub(super) struct Project;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl GrpcVisitor for Project {
    type Entry = Option<Codec>;
    fn unary<O: WireOperation>(self) -> Self::Entry
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
    {
        Some(Codec {
            decode: decode::<O::Request>,
            encode: encode::<O::Response>,
            #[cfg(feature = "streaming")]
            input: None,
            #[cfg(feature = "streaming")]
            output: None,
        })
    }
    fn upload<O: WireOperation + InputDeclaration>(self) -> Self::Entry
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        O::Item: Protobuf + MemorySize,
    {
        #[cfg(feature = "streaming")]
        {
            Some(Codec {
                input: Some(item::<O::Item>()),
                ..self.unary::<O>()?
            })
        }
        #[cfg(not(feature = "streaming"))]
        {
            None
        }
    }
    fn download<O: WireOperation + OutputDeclaration>(self) -> Self::Entry
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        O::Item: Protobuf + MemorySize,
    {
        #[cfg(feature = "streaming")]
        {
            Some(Codec {
                output: Some(item::<O::Item>()),
                ..self.unary::<O>()?
            })
        }
        #[cfg(not(feature = "streaming"))]
        {
            None
        }
    }
    fn exchange<O: WireOperation + InputDeclaration + OutputDeclaration>(self) -> Self::Entry
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        <O as InputDeclaration>::Item: Protobuf + MemorySize,
        <O as OutputDeclaration>::Item: Protobuf + MemorySize,
    {
        #[cfg(feature = "streaming")]
        {
            Some(Codec {
                input: Some(item::<<O as InputDeclaration>::Item>()),
                output: Some(item::<<O as OutputDeclaration>::Item>()),
                ..self.unary::<O>()?
            })
        }
        #[cfg(not(feature = "streaming"))]
        {
            None
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "streaming")]
fn item<T: Protobuf + MemorySize + 'static>() -> ItemCodec {
    ItemCodec {
        item_type: std::any::TypeId::of::<T>(),
        memory: T::decoding_memory,
        decode: decode::<T>,
        length: |value| {
            value
                .downcast_ref::<T>()
                .map(Protobuf::encoded_len)
                .ok_or_else(|| {
                    MessageError::borrowed("protobuf stream item type differs from operation")
                })
        },
        encode: encode::<T>,
    }
}

fn decode<T: Protobuf + 'static>(payload: &[u8], target: &mut dyn Any) -> Result<(), MessageError> {
    let target = target
        .downcast_mut::<Option<T>>()
        .ok_or_else(|| MessageError::borrowed("protobuf request type differs from operation"))?;
    T::decoding_memory(payload)?;
    *target = Some(T::decode(payload)?);
    Ok(())
}

fn encode<T: Protobuf + 'static>(value: &dyn Any) -> Result<Vec<u8>, MessageError> {
    let value = value
        .downcast_ref::<T>()
        .ok_or_else(|| MessageError::borrowed("protobuf response type differs from operation"))?;
    Ok(value.encode_to_vec())
}

/* === Tests === */
