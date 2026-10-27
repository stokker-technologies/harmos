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
use crate::journal::Position;
use crate::message::{PostcardReader, PostcardValue, decoding_add};
use crate::schema::declaration::{self, Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{MemorySize, Message, MessageError};

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
context_shape!(RecordContext, &[Field { name: Cow::Borrowed("position"), tag: 1, kind: Kind::U64, cardinality: Cardinality::Required }]);
context_shape!(
    StreamContext,
    &[
        Field { name: Cow::Borrowed("sequence"), tag: 1, kind: Kind::U64, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("missed"), tag: 2, kind: Kind::U64, cardinality: Cardinality::Required },
    ]
);
context_shape!(DeliveryContext, CONTEXT_FIELDS);

/* === Type Aliases === */

/* === Structs === */

// One Delivered Row Of Listener `L`. Each Listener Is A Distinct Type, So Its
// Delivery Is Its Callback Operation's Request.
pub struct Callback<L: Listener>(std::marker::PhantomData<L>);

pub struct Delivery<L: Listener> {
    context: Context,
    row: L::Row,
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

// A Listener Declaration: The Row It Receives And Its Callback Operation.
pub trait Listener: Send + Sync + 'static {
    type Row: crate::Portable;
    const CALLBACK: &'static str;
    const POLICY: crate::operations::OperationPolicy;
}

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

impl<L: Listener> Delivery<L> {
    pub fn new(context: Context, row: L::Row) -> Result<Self, ContextError> {
        context.validate()?;
        Ok(Self { context, row })
    }

    pub fn context(&self) -> Context {
        self.context
    }

    pub fn row(&self) -> &L::Row {
        &self.row
    }

    pub fn into_row(self) -> L::Row {
        self.row
    }
}

impl<L: Listener> Clone for Delivery<L>
where
    L::Row: Clone,
{
    fn clone(&self) -> Self {
        Self { context: self.context, row: self.row.clone() }
    }
}

impl<L: Listener> std::fmt::Debug for Delivery<L>
where
    L::Row: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Delivery").field("context", &self.context).field("row", &self.row).finish()
    }
}

impl<L: Listener> PartialEq for Delivery<L>
where
    L::Row: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.row == other.row
    }
}

impl<L: Listener> Message for Delivery<L> {
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
        let row = <L::Row as Message>::decoding_memory(input.remaining())?;
        decoding_add(std::mem::size_of::<Self>(), row.saturating_sub(std::mem::size_of::<L::Row>()))
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <Self as Message>::decoding_memory(bytes)?;
        let mut input = PostcardReader::new(bytes);
        let context = <Context as PostcardValue>::read(&mut input)?;
        let row = <L::Row as Message>::decode(input.remaining())?;
        Ok(Self { context, row })
    }
}

impl<L: Listener> crate::operations::OperationDeclaration for Callback<L> {
    type Request = Delivery<L>;
    type Response = ();
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = L::CALLBACK;
    const POLICY: crate::operations::OperationPolicy = L::POLICY;
    // Every Listener Callback Is Shared: The Definition Refuses Any Other
    const VISIBILITY: crate::operations::Visibility = crate::operations::Visibility::Shared;
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
                1 => Self::Stream { sequence: input.value()?, missed: input.value()? },
                _ => return Err(MessageError::borrowed("unknown listener source context")),
            };
            context.validate().map_err(|_| MessageError::borrowed("invalid listener source context"))?;
            Ok(context)
        })
    }
}

impl<L: Listener> MemorySize for Delivery<L> {
    fn memory_size(&self) -> usize {
        let row = self.row.memory_size();
        if row == usize::MAX {
            return usize::MAX;
        }
        std::mem::size_of::<Self>().saturating_add(row.saturating_sub(std::mem::size_of::<L::Row>()))
    }
}

impl<L: Listener> Described for Delivery<L> {
    const NAME: &'static str = declaration::Name::new("harmos::Delivery", &[<L::Row as Described>::NAME]).as_str();
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
            kind: Kind::Message { definition: <L::Row as Described>::DEFINITION },
            cardinality: Cardinality::Required,
        },
    ];
    const MARKER: &'static [u8] =
        declaration::encode_buffer::<2048>(Self::NAME, Self::FIELDS).split_at(declaration::encoded_len(Self::NAME, Self::FIELDS)).0;
}

/* === Public Functions === */

pub(super) fn matches(
    request: crate::schema::graph::View<'_>, row: &crate::schema::graph::Schema, settings: &crate::config::SchemaSettings,
) -> Result<bool, crate::schema::graph::Error> {
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
    let expected = crate::schema::graph::Schema::from_declaration(DeliveryContext::DEFINITION, settings)?;
    Ok(request.schema().equivalent(context.index(), &expected, expected.root, settings)?
        && request.schema().equivalent(delivered.index(), row, row.root, settings)?)
}

/* === Private Functions === */

/* === Tests === */
