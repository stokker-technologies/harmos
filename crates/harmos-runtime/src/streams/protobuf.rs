/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protobuf Rows Projected Into Canonical Stream Storage

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;
use prost_reflect::{DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, ReflectMessage, Value};

// Local Imports
use crate::plugin::grpc::Error;
use harmos_domain::message::postcard::MAX_VALUES;
use harmos_domain::schema::graph::Schema;
use harmos_operations::execution::grpc::{DecodeInput, Dynamic};
use harmos_protocol::columns::batch;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Row {
    pub(crate) descriptor: MessageDescriptor,
    pub(crate) schema: Schema,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Row {
    pub(crate) fn encode(&self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        self.codec().encode(payload, &mut bytes).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(bytes)
    }

    fn codec(&self) -> Dynamic<'_> {
        Dynamic::new(self.descriptor.clone(), &self.schema)
    }

    pub(crate) fn new(descriptor: MessageDescriptor) -> Result<Self, Error> {
        let schema = Dynamic::schema(&descriptor).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Self { descriptor, schema })
    }

    pub(crate) fn decode(&self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        self.codec().decode(DecodeInput::new(payload)).map_err(|error| Error::Invalid(error.to_string()))
    }

    /// Convert One Batch Part Whose Every Column Holds Exactly `count` Values Into Stored Rows.
    ///
    /// A part larger than one stored row may hold is stored as consecutive rows, each with
    /// its own first-sample time and count; the host slices columns, never samples.
    pub(crate) fn decode_batch(&self, payload: &[u8], count: u64, period_nanos: u64) -> Result<Vec<(i64, u64, Vec<u8>)>, Error> {
        let invalid = || Error::Invalid("Answer Sample Window: the stored row is not a batch".into());
        let message = DynamicMessage::decode(self.descriptor.clone(), payload).map_err(|error| Error::Invalid(error.to_string()))?;
        let start = message.get_field_by_number(batch::START_UNIX_NANOS).and_then(|value| value.as_i64()).ok_or_else(invalid)?;
        let columns = message.get_field_by_number(batch::COLUMNS).ok_or_else(invalid)?;
        let Value::Message(columns) = columns.as_ref() else {
            return Err(invalid());
        };
        let fields = columns.descriptor().fields().collect::<Vec<_>>();
        for field in &fields {
            let length = match columns.get_field(field).as_ref() {
                Value::List(values) => values.len(),
                _ => 0,
            };
            if length as u64 != count {
                return Err(Error::Invalid(format!(
                    "Answer Sample Window: column `{}` holds {length} values for {count} samples; send one value per sample in every column",
                    field.name()
                )));
            }
        }
        let chunk = chunk(&fields);
        // A Part Within One Row's Limit Is Stored as Decoded, Without Copying Its Columns.
        if count <= chunk {
            let bytes = self.decode(payload)?;
            return Ok(vec![(start, count, bytes)]);
        }
        let mut rows = Vec::new();
        let mut offset = 0_u64;
        while offset < count {
            let length = chunk.min(count - offset);
            let first = i64::try_from(i128::from(start) + i128::from(offset) * i128::from(period_nanos))
                .map_err(|_| Error::Invalid("Grant Sample Window: sample times must fall before 2262".into()))?;
            let range = offset as usize..(offset + length) as usize;
            let mut part = DynamicMessage::new(columns.descriptor());
            for field in &fields {
                if let Value::List(values) = columns.get_field(field).as_ref() {
                    part.set_field(field, Value::List(values[range.clone()].to_vec()));
                }
            }
            let mut stored = DynamicMessage::new(self.descriptor.clone());
            stored.set_field_by_number(batch::START_UNIX_NANOS, Value::I64(first));
            stored.set_field_by_number(batch::PERIOD_NANOS, Value::U64(period_nanos));
            stored.set_field_by_number(batch::COUNT, Value::U64(length));
            stored.set_field_by_number(batch::COLUMNS, Value::Message(part));
            let bytes = self.decode(&stored.encode_to_vec())?;
            rows.push((first, length, bytes));
            offset += length;
        }
        Ok(rows)
    }

    /// The Fewest Stored Rows and Payload Bytes One Window of `count` Samples Takes, Every Value at Its Smallest.
    pub(crate) fn least(&self, count: u64) -> (u64, u128) {
        let Some(Kind::Message(columns)) = self.descriptor.get_field(batch::COLUMNS).map(|field| field.kind()) else {
            return (count, u128::from(count));
        };
        let fields = columns.fields().collect::<Vec<_>>();
        let sample = fields.iter().map(|field| smallest(&field.kind(), 0)).sum::<usize>().max(1);
        (count.div_ceil(chunk(&fields)), u128::from(count) * sample as u128)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// Samples One Stored Row Holds: Each Claims Its Values in Every Column; the Header and List Lengths Claim the Rest.
fn chunk(fields: &[FieldDescriptor]) -> u64 {
    let per_sample = fields.iter().map(|field| 1 + cost(&field.kind(), 0)).sum::<usize>().max(1);
    ((MAX_VALUES - 64) / per_sample).max(1) as u64
}

/// The Fewest Stored Bytes One Value of `kind` Encodes To: Floats Are Fixed, Everything Else at Least One Byte.
fn smallest(kind: &Kind, depth: usize) -> usize {
    match kind {
        Kind::Double => 8,
        Kind::Float => 4,
        Kind::Message(message) if depth < 8 => message.fields().map(|field| smallest(&field.kind(), depth + 1)).sum::<usize>(),
        _ => 1,
    }
}

/// Structural Values One Element of `kind` Claims When Stored: Presence, Nested Fields and List Lengths Included.
fn cost(kind: &Kind, depth: usize) -> usize {
    let Kind::Message(message) = kind else {
        return 1;
    };
    if depth >= 8 {
        return 64;
    }
    message.fields().map(|field| 2 + cost(&field.kind(), depth + 1)).sum::<usize>() + 1
}

/* === Tests === */
