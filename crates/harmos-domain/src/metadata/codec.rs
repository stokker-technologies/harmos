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
use crate::schema::declaration::{Definition, Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::value::codec::{
    ENTRY_DEFINITION, TIMESTAMP_DEFINITION, VALUE_DEPTH, entries_memory, entries_size, read_entries, read_timestamp, timestamp_size,
    write_entries, write_timestamp,
};

/* === Constants === */
const IDENTIFIER_FIELDS: &[Field] =
    &[Field { name: Cow::Borrowed("value"), tag: 1, kind: Kind::String, cardinality: Cardinality::Required }];

const METADATA_FIELDS: &[Field] = &[
    field("identifier", 1, Kind::String, Cardinality::Required),
    field("label", 2, Kind::String, Cardinality::Required),
    field("description", 3, Kind::String, Cardinality::Optional),
    field("tags", 4, Kind::String, Cardinality::Repeated),
    field("attributes", 5, message(ENTRY_DEFINITION), Cardinality::Repeated),
    field("created_at", 6, message(TIMESTAMP_DEFINITION), Cardinality::Optional),
    field("updated_at", 7, message(TIMESTAMP_DEFINITION), Cardinality::Optional),
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Described for Identifier {
    const NAME: &'static str = "harmos::Identifier";
    const FIELDS: &'static [Field] = IDENTIFIER_FIELDS;
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for Metadata {
    const NAME: &'static str = "harmos::Metadata";
    const FIELDS: &'static [Field] = METADATA_FIELDS;
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
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
                Option::<String>::memory(input)?.saturating_sub(std::mem::size_of::<Option<String>>()),
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

/* === Public Functions === */

/* === Private Functions === */
const fn field(name: &'static str, tag: u32, kind: Kind, cardinality: Cardinality) -> Field {
    Field { name: Cow::Borrowed(name), tag, kind, cardinality }
}

const fn message(definition: Definition) -> Kind {
    Kind::Message { definition }
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
    if bool::read(input)? { read_timestamp(input).map(Some) } else { Ok(None) }
}

/* === Tests === */
