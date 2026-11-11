/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "encoding.rs"]
mod encoding;

/* === Re-Exports === */

pub use encoding::{EncodingError, encode_borrowed, encoding_memory_bytes};

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
// Third Party Imports

// Local Imports
use crate::{MemorySize, Message, MessageError};
use harmos_domain::__private::{PostcardReader, PostcardValue, decoding_add};
use harmos_domain::journal::Position;
use harmos_domain::schema::declaration::{self, Field, Kind};
use harmos_domain::schema::{Cardinality, Described};

/* === Constants === */

const CONTEXT_FIELDS: &[Field] = &[
    Field {
        name: Cow::Borrowed("record"),
        tag: 1,
        kind: Kind::Message { definition: RecordContext::DEFINITION },
        cardinality: Cardinality::Variant,
    },
    Field {
        name: Cow::Borrowed("stream"),
        tag: 2,
        kind: Kind::Message { definition: StreamContext::DEFINITION },
        cardinality: Cardinality::Variant,
    },
];

/* === Macros === */

macro_rules! context_shape {
    ($name:ident, $fields:expr) => {
        struct $name;
        impl Described for $name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($name));
            const FIELDS: &'static [Field] = $fields;
            const MARKER: &'static [u8] =
                &declaration::encode::<{ declaration::encoded_len(Self::NAME, Self::FIELDS) }>(Self::NAME, Self::FIELDS);
        }
    };
}
context_shape!(
    RecordContext,
    &[Field {
        name: Cow::Borrowed("position"),
        tag: 1,
        kind: Kind::U64,
        cardinality: Cardinality::Required
    }]
);
context_shape!(
    StreamContext,
    &[
        Field {
            name: Cow::Borrowed("sequence"),
            tag: 1,
            kind: Kind::U64,
            cardinality: Cardinality::Required
        },
        Field {
            name: Cow::Borrowed("missed"),
            tag: 2,
            kind: Kind::U64,
            cardinality: Cardinality::Required
        },
    ]
);
context_shape!(DeliveryContext, CONTEXT_FIELDS);

/* === Type Aliases === */

/* === Structs === */

pub struct Delivery<R: harmos_domain::Portable> {
    context: Context,
    row: R,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Record { position: Position },
    Stream { sequence: u64, missed: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("a delivered record must have a positive journal position")]
    Position,

    #[error("a delivered stream row requires a positive logical sequence with missed < sequence")]
    Sequence,
}

/* === Traits === */

/* === Implementations === */

impl Context {
    pub fn validate(self) -> Result<(), ContextError> {
        match self {
            Self::Record { position: Position(0) } => Err(ContextError::Position),
            Self::Stream { sequence, missed } if sequence == 0 || missed >= sequence => Err(ContextError::Sequence),
            _ => Ok(()),
        }
    }
}

impl<R: harmos_domain::Portable> Delivery<R> {
    pub fn new(context: Context, row: R) -> Result<Self, ContextError> {
        context.validate()?;
        Ok(Self { context, row })
    }

    pub fn context(&self) -> Context {
        self.context
    }

    pub fn row(&self) -> &R {
        &self.row
    }

    pub fn into_row(self) -> R {
        self.row
    }
}

impl<R: harmos_domain::Portable> Clone for Delivery<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            context: self.context,
            row: self.row.clone(),
        }
    }
}

impl<R: harmos_domain::Portable> std::fmt::Debug for Delivery<R>
where
    R: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Delivery").field("context", &self.context).field("row", &self.row).finish()
    }
}

impl<R: harmos_domain::Portable> PartialEq for Delivery<R>
where
    R: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.row == other.row
    }
}

impl<R: harmos_domain::Portable> Message for Delivery<R> {
    const CODEC: &'static str = "harmos.postcard.v1";
    fn encode(&self) -> Vec<u8> {
        encoding::encode_borrowed(self.context, &self.row, usize::MAX).expect("validated delivery and cooperative message encoder")
    }
    fn encoded_size(&self) -> usize {
        encoding::envelope_len(self.context, self.row.encoded_size()).unwrap_or(usize::MAX)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        let mut input = PostcardReader::new(bytes);
        let _ = <Context as PostcardValue>::read(&mut input)?;
        let row = <R as Message>::decoding_memory(input.remaining())?;
        decoding_add(std::mem::size_of::<Self>(), row.saturating_sub(std::mem::size_of::<R>()))
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <Self as Message>::decoding_memory(bytes)?;
        let mut input = PostcardReader::new(bytes);
        let context = <Context as PostcardValue>::read(&mut input)?;
        let row = <R as Message>::decode(input.remaining())?;
        Ok(Self { context, row })
    }
}

impl PostcardValue for Context {
    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Record { position } => {
                0u32.write(bytes);
                position.0.write(bytes);
            }
            Self::Stream { sequence, missed } => {
                1u32.write(bytes);
                sequence.write(bytes);
                missed.write(bytes);
            }
        }
    }
    fn size(&self) -> usize {
        match self {
            Self::Record { position } => 1usize.saturating_add(position.0.size()),
            Self::Stream { sequence, missed } => 1usize.saturating_add(sequence.size()).saturating_add(missed.size()),
        }
    }
    fn memory(input: &mut PostcardReader<'_>) -> Result<usize, MessageError> {
        <Self as PostcardValue>::read(input)?;
        Ok(std::mem::size_of::<Self>())
    }
    fn read(input: &mut PostcardReader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            let context = match input.value::<u32>()? {
                0 => Self::Record { position: Position(input.value()?) },
                1 => Self::Stream {
                    sequence: input.value()?,
                    missed: input.value()?,
                },
                _ => return Err(MessageError::borrowed("unknown listener source context")),
            };
            context.validate().map_err(|_| MessageError::borrowed("invalid listener source context"))?;
            Ok(context)
        })
    }
}

impl<R: harmos_domain::Portable> MemorySize for Delivery<R> {
    fn memory_size(&self) -> usize {
        let row = self.row.memory_size();
        if row == usize::MAX {
            return usize::MAX;
        }
        std::mem::size_of::<Self>().saturating_add(row.saturating_sub(std::mem::size_of::<R>()))
    }
}

impl<R: harmos_domain::Portable> Described for Delivery<R> {
    const NAME: &'static str = declaration::Name::new("harmos::Delivery", &[<R as Described>::NAME]).as_str();
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("context"),
            tag: 1,
            kind: Kind::Message { definition: DeliveryContext::DEFINITION },
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("row"),
            tag: 2,
            kind: Kind::Message { definition: <R as Described>::DEFINITION },
            cardinality: Cardinality::Required,
        },
    ];
    const MARKER: &'static [u8] =
        declaration::encode_buffer::<2048>(Self::NAME, Self::FIELDS).split_at(declaration::encoded_len(Self::NAME, Self::FIELDS)).0;
}

/* === Public Functions === */

pub(super) fn matches(
    request: harmos_domain::schema::graph::View<'_>, row: &harmos_domain::schema::graph::Schema,
    settings: &harmos_domain::config::SchemaSettings,
) -> Result<bool, harmos_domain::schema::graph::Error> {
    let fields = request.fields();
    if fields.len() != 2
        || fields[0].name != "context"
        || fields[0].tag != 1
        || fields[1].name != "row"
        || fields[1].tag != 2
        || fields.iter().any(|field| field.cardinality != Cardinality::Required)
    {
        return Ok(false);
    }
    let Some(context) = request.message(&fields[0])? else {
        return Ok(false);
    };
    let Some(delivered) = request.message(&fields[1])? else {
        return Ok(false);
    };
    let expected = harmos_domain::schema::graph::Schema::from_declaration(DeliveryContext::DEFINITION, settings)?;
    Ok(request.schema().equivalent(context.index(), &expected, expected.root, settings)?
        && request.schema().equivalent(delivered.index(), row, row.root, settings)?)
}

/* === Private Functions === */

/* === Tests === */
