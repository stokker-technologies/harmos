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
use std::borrow::Cow;
// Third Party Imports

// Local Imports
use super::{Identifier, Metadata};
use crate::MessageError;
use crate::message::{PostcardReader as Reader, PostcardValue, decoding_add};
use crate::schema::{Cardinality, Described, Field, Kind};
use crate::value::codec::{
    ENTRY_FIELDS, TIMESTAMP_FIELDS, VALUE_DEPTH, entries_memory, entries_size, read_entries,
    read_timestamp, timestamp_size, write_entries, write_timestamp,
};

/* === Constants === */
const IDENTIFIER_FIELDS: &[Field] = &[Field {
    name: Cow::Borrowed("value"),
    tag: 1,
    kind: Kind::String,
    cardinality: Cardinality::Required,
}];

const METADATA_FIELDS: &[Field] = &[
    field("identifier", 1, Kind::String, Cardinality::Required),
    field("label", 2, Kind::String, Cardinality::Required),
    field("description", 3, Kind::String, Cardinality::Optional),
    field("tags", 4, Kind::String, Cardinality::Repeated),
    field(
        "attributes",
        5,
        message(ENTRY_FIELDS),
        Cardinality::Repeated,
    ),
    field(
        "created_at",
        6,
        message(TIMESTAMP_FIELDS),
        Cardinality::Optional,
    ),
    field(
        "updated_at",
        7,
        message(TIMESTAMP_FIELDS),
        Cardinality::Optional,
    ),
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Described for Identifier {
    const FIELDS: &'static [Field] = IDENTIFIER_FIELDS;
}

impl Described for Metadata {
    const FIELDS: &'static [Field] = METADATA_FIELDS;
}

impl PostcardValue for Metadata {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.identifier.write(bytes);
        self.label.write(bytes);
        self.description.write(bytes);
        self.tags.write(bytes);
        write_entries(&self.attributes, bytes);
        write_instant(self.created_at, bytes);
        write_instant(self.updated_at, bytes);
    }

    fn size(&self) -> usize {
        self.identifier
            .size()
            .saturating_add(self.label.size())
            .saturating_add(self.description.size())
            .saturating_add(self.tags.size())
            .saturating_add(entries_size(&self.attributes))
            .saturating_add(instant_size(self.created_at))
            .saturating_add(instant_size(self.updated_at))
    }

    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        input.nested(|input| {
            let mut owned = std::mem::size_of::<Self>();
            for allocation in [
                Identifier::memory(input)?.saturating_sub(std::mem::size_of::<Identifier>()),
                String::memory(input)?.saturating_sub(std::mem::size_of::<String>()),
                Option::<String>::memory(input)?
                    .saturating_sub(std::mem::size_of::<Option<String>>()),
                Vec::<String>::memory(input)?.saturating_sub(std::mem::size_of::<Vec<String>>()),
                entries_memory(input, VALUE_DEPTH)?,
            ] {
                owned = decoding_add(owned, allocation)?;
            }
            read_instant(input)?;
            read_instant(input)?;
            Ok(owned)
        })
    }

    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        input.nested(|input| {
            let identifier = Identifier::read(input)?;
            let label = String::read(input)?;
            let description = Option::<String>::read(input)?;
            let tags = Vec::<String>::read(input)?;
            let attributes = read_entries(input, VALUE_DEPTH)?;
            let created_at = read_instant(input)?;
            let updated_at = read_instant(input)?;
            let mut metadata = Self::definition(identifier, label, description, tags);
            metadata.tags.shrink_to_fit();
            metadata.attributes = attributes;
            metadata.created_at = created_at;
            metadata.updated_at = updated_at;
            Ok(metadata)
        })
    }
}

crate::with_protobuf! {
    impl crate::Protobuf for Identifier {
        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(crate::Protobuf::encoded_len(self));
            crate::Protobuf::encode_into(self, &mut bytes);
            bytes
        }
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            if !self.0.is_empty() {
                prost::encoding::string::encode(1, &self.0, bytes);
            }
        }
        fn encoded_len(&self) -> usize {
            if self.0.is_empty() {
                0
            } else {
                prost::encoding::string::encoded_len(1, &self.0)
            }
        }
        fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
            Self::decoding_memory_input(crate::message::DecodeInput::new(bytes))
        }
        fn decoding_memory_input(input: crate::message::DecodeInput<'_>) -> Result<usize, MessageError> {
            let text = protobuf::identifier(input)?;
            decoding_add(std::mem::size_of::<Self>(), text.len())
        }
        fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
            Self::decode_input(crate::message::DecodeInput::new(bytes))
        }
        fn decode_input(input: crate::message::DecodeInput<'_>) -> Result<Self, MessageError> {
            Ok(Self(protobuf::identifier(input)?.to_owned()))
        }
    }

    impl crate::Protobuf for Metadata {
        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(crate::Protobuf::encoded_len(self));
            crate::Protobuf::encode_into(self, &mut bytes);
            bytes
        }
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            protobuf::encode(self, bytes);
        }
        fn encoded_len(&self) -> usize {
            protobuf::body_len(self)
        }
        fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
            Self::decoding_memory_input(crate::message::DecodeInput::new(bytes))
        }
        fn decoding_memory_input(input: crate::message::DecodeInput<'_>) -> Result<usize, MessageError> {
            protobuf::measure(input)
        }
        fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
            Self::decode_input(crate::message::DecodeInput::new(bytes))
        }
        fn decode_input(input: crate::message::DecodeInput<'_>) -> Result<Self, MessageError> {
            protobuf::measure(input)?;
            protobuf::read(input)
        }
    }

        mod protobuf {
        use prost::encoding;

        use super::{Metadata, super::validate_identifier};
        use crate::message::{DecodeInput, decoding_add};
        use crate::value::codec::VALUE_DEPTH;
        use crate::value::codec::protobuf::{
            encode_entries_at, encode_timestamp, entries_len_at, framed, measure_entries_at, message,
            read_entries_at, timestamp, timestamp_len,
        };
        use crate::MessageError;

        pub(super) fn identifier<'a>(input: DecodeInput<'a>) -> Result<&'a str, MessageError> {
            let mut value = "";
            input.fields(&mut |field| {
                if field.tag == 1 {
                    value = field.text()?;
                }
                Ok(())
            })?;
            validate_identifier(value).map_err(|_| MessageError::borrowed("invalid identifier"))?;
            Ok(value)
        }

        pub(super) fn encode(metadata: &Metadata, bytes: &mut Vec<u8>) {
            encoding::string::encode(1, &metadata.identifier.0, bytes);
            encoding::string::encode(2, &metadata.label, bytes);
            if let Some(description) = &metadata.description {
                encoding::string::encode(3, description, bytes);
            }
            encoding::string::encode_repeated(4, &metadata.tags, bytes);
            encode_entries_at(5, &metadata.attributes, bytes);
            for (tag, instant) in [(6, metadata.created_at), (7, metadata.updated_at)] {
                if let Some(instant) = instant {
                    message(tag, timestamp_len(instant), bytes);
                    encode_timestamp(instant, bytes);
                }
            }
        }

        pub(super) fn body_len(metadata: &Metadata) -> usize {
            let mut length = encoding::string::encoded_len(1, &metadata.identifier.0)
                .saturating_add(encoding::string::encoded_len(2, &metadata.label))
                .saturating_add(
                    metadata
                        .description
                        .as_ref()
                        .map_or(0, |description| encoding::string::encoded_len(3, description)),
                )
                .saturating_add(encoding::string::encoded_len_repeated(4, &metadata.tags))
                .saturating_add(entries_len_at(5, &metadata.attributes));
            for (tag, instant) in [(6, metadata.created_at), (7, metadata.updated_at)] {
                if let Some(instant) = instant {
                    length = length.saturating_add(framed(tag, timestamp_len(instant)));
                }
            }
            length
        }

        pub(super) fn measure(input: DecodeInput<'_>) -> Result<usize, MessageError> {
            let mut owned = std::mem::size_of::<Metadata>();
            let mut tags = 0usize;
            input.fields(&mut |field| {
                match field.tag {
                    1..=3 => owned = decoding_add(owned, field.text()?.len())?,
                    4 => {
                        tags = decoding_add(tags, 1)?;
                        owned = decoding_add(owned, field.text()?.len())?;
                    }
                    6 | 7 => {
                        timestamp(input.child(field.bytes()?)?)?;
                    }
                    _ => {}
                }
                Ok(())
            })?;
            identifier(input)?;
            owned = decoding_add(owned, crate::message::decoding_multiply(tags, std::mem::size_of::<String>())?)?;
            decoding_add(owned, measure_entries_at(input, 5, VALUE_DEPTH)?)
        }

        pub(super) fn read(input: DecodeInput<'_>) -> Result<Metadata, MessageError> {
            let identifier = super::Identifier(identifier(input)?.to_owned());
            let mut label = String::new();
            let mut description = None;
            let mut tags = Vec::new();
            let mut created_at = None;
            let mut updated_at = None;
            input.fields(&mut |field| {
                match field.tag {
                    2 => label = field.text()?.to_owned(),
                    3 => description = Some(field.text()?.to_owned()),
                    4 => tags.push(field.text()?.to_owned()),
                    6 => created_at = Some(timestamp(input.child(field.bytes()?)?)?),
                    7 => updated_at = Some(timestamp(input.child(field.bytes()?)?)?),
                    _ => {}
                }
                Ok(())
            })?;
            let mut metadata = Metadata::definition(identifier, label, description, tags);
            metadata.tags.shrink_to_fit();
            metadata.attributes = read_entries_at(input, 5, VALUE_DEPTH)?;
            metadata.created_at = created_at;
            metadata.updated_at = updated_at;
            Ok(metadata)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
const fn field(name: &'static str, tag: u32, kind: Kind, cardinality: Cardinality) -> Field {
    Field {
        name: Cow::Borrowed(name),
        tag,
        kind,
        cardinality,
    }
}

const fn message(fields: &'static [Field]) -> Kind {
    Kind::Message {
        fields: Cow::Borrowed(fields),
    }
}

fn write_instant(value: Option<time::OffsetDateTime>, bytes: &mut Vec<u8>) {
    value.is_some().write(bytes);
    if let Some(value) = value {
        write_timestamp(value, bytes);
    }
}

fn instant_size(value: Option<time::OffsetDateTime>) -> usize {
    1usize.saturating_add(value.map_or(0, timestamp_size))
}

fn read_instant(input: &mut Reader<'_>) -> Result<Option<time::OffsetDateTime>, MessageError> {
    if bool::read(input)? {
        read_timestamp(input).map(Some)
    } else {
        Ok(None)
    }
}

/* === Tests === */
