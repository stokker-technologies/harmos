/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Interval Window Columns: Transpose One-Sample Rows and Split Parts at the Message Limit

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::result;

// Third Party Imports
#[cfg(not(target_arch = "wasm32"))]
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MessageDescriptor, Value};
use tonic::Status;

// Local Imports
use crate::message;
#[cfg(not(target_arch = "wasm32"))]
use harmos_protocol::plugin::{InstanceSamplesResponse, SampleBatch, instance_samples_response::Frame};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Row's Descriptor and the Columns Derived From It.
#[derive(Clone)]
pub(crate) struct Columns {
    pub row: MessageDescriptor,
    pub columns: MessageDescriptor,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Columns {
    /// Derive the Columns of `T`; an Interval Row Holds One Singular Value per Field.
    pub fn of<T: message::Message>() -> result::Result<Self, String> {
        let row = descriptor(T::types(), &T::name())?;
        let columns = harmos_protocol::columns::derive(&row)?.columns;
        Ok(Self { row, columns })
    }

    /// Check That `C` Repeats Every Field of the Row With Its Number and Type.
    pub fn check<C: message::Message>(&self) -> result::Result<(), String> {
        let authored = descriptor(C::types(), &C::name())?;
        let matches = authored.fields().len() == self.row.fields().len()
            && self.row.fields().all(|field| {
                let Some(column) = authored.get_field(field.number()) else {
                    return false;
                };
                if !column.is_list() {
                    return false;
                }
                if !field.field_descriptor_proto().proto3_optional.unwrap_or(false) {
                    return format!("{:?}", column.kind()) == format!("{:?}", field.kind());
                }
                let Kind::Message(element) = column.kind() else {
                    return false;
                };
                let Some(value) = element.get_field(1) else {
                    return false;
                };
                element.fields().len() == 1
                    && value.supports_presence()
                    && !value.is_list()
                    && format!("{:?}", value.kind()) == format!("{:?}", field.kind())
            });
        if !matches {
            return Err(format!(
                "Declare Window Observer: columns `{}` must repeat every field of row `{}` with the same number and type",
                C::name(),
                self.row.full_name()
            ));
        }
        Ok(())
    }

    /// Transpose Encoded Rows Into One Columns Message, Defaults Included.
    pub fn transpose(&self, rows: &[Vec<u8>]) -> result::Result<DynamicMessage, Status> {
        let fields = self.row.fields().collect::<Vec<_>>();
        let mut values = vec![Vec::with_capacity(rows.len()); fields.len()];
        for row in rows {
            let row = DynamicMessage::decode(self.row.clone(), row.as_slice())
                .map_err(|error| Status::internal(format!("Answer Sample Window: decode an observer row: {error}")))?;
            for (column, field) in values.iter_mut().zip(&fields) {
                let value = if field.field_descriptor_proto().proto3_optional.unwrap_or(false) {
                    let Kind::Message(element) = self.columns.get_field(field.number()).unwrap().kind() else {
                        unreachable!("optional column element")
                    };
                    let mut element = DynamicMessage::new(element);
                    if row.has_field(field) {
                        element.set_field_by_number(1, row.get_field(field).into_owned());
                    }
                    Value::Message(element)
                } else {
                    row.get_field(field).into_owned()
                };
                column.push(value);
            }
        }
        let mut columns = DynamicMessage::new(self.columns.clone());
        for (column, field) in values.into_iter().zip(&fields) {
            columns.set_field_by_number(field.number(), Value::List(column));
        }
        Ok(columns)
    }

    /// Decode a Window Observer's Columns for Splitting.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode(&self, columns: &[u8]) -> result::Result<DynamicMessage, Status> {
        DynamicMessage::decode(self.columns.clone(), columns)
            .map_err(|error| Status::internal(format!("Answer Sample Window: decode the observer's columns: {error}")))
    }
}

/* === Public Functions === */

/// Split `count` Samples Into Parts Whose Encoded Responses Each Fit `limit` Bytes.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parts(columns: &Columns, encoded: Vec<u8>, count: u64, limit: usize) -> result::Result<Vec<InstanceSamplesResponse>, Status> {
    let whole = response(0, count, encoded, true);
    if whole.encoded_len() <= limit {
        return Ok(vec![whole]);
    }
    let Some(Frame::Batch(batch)) = whole.frame else {
        unreachable!("a batch response")
    };
    let message = columns.decode(&batch.columns)?;
    let mut parts = Vec::new();
    split(columns, &message, 0, count, count, limit, &mut parts)?;
    Ok(parts)
}

/* === Private Functions === */

fn descriptor(types: prost_types::FileDescriptorSet, name: &str) -> result::Result<MessageDescriptor, String> {
    let pool = DescriptorPool::from_file_descriptor_set(types).map_err(|error| format!("Declare Observer: {error}"))?;
    pool.get_message_by_name(name).ok_or_else(|| format!("Declare Observer: `{name}` is missing from its own descriptors"))
}

#[cfg(not(target_arch = "wasm32"))]
fn response(offset: u64, count: u64, columns: Vec<u8>, last: bool) -> InstanceSamplesResponse {
    InstanceSamplesResponse {
        samples: Vec::new(),
        frame: Some(Frame::Batch(SampleBatch { offset, count, columns, last })),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn split(
    columns: &Columns, message: &DynamicMessage, offset: u64, count: u64, total: u64, limit: usize,
    parts: &mut Vec<InstanceSamplesResponse>,
) -> result::Result<(), Status> {
    let mut slice = DynamicMessage::new(columns.columns.clone());
    for field in columns.columns.fields() {
        if let Value::List(values) = message.get_field(&field).as_ref() {
            let values = values[offset as usize..(offset + count) as usize].to_vec();
            slice.set_field(&field, Value::List(values));
        }
    }
    let part = response(offset, count, slice.encode_to_vec(), offset + count == total);
    if part.encoded_len() <= limit {
        parts.push(part);
        return Ok(());
    }
    if count == 1 {
        return Err(Status::resource_exhausted(format!(
            "Answer Sample Window: one `{}` sample exceeds the {limit}-byte message limit; shrink the row or raise the transport's message size",
            columns.row.full_name()
        )));
    }
    let half = count / 2;
    split(columns, message, offset, half, total, limit, parts)?;
    split(columns, message, offset + half, count - half, total, limit, parts)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::ReflectMessage;

    #[derive(crate::Message)]
    struct Reading {
        value: f64,
        label: String,
    }

    #[derive(crate::Message)]
    struct Readings {
        value: Vec<f64>,
        label: Vec<String>,
    }

    #[derive(crate::Message)]
    struct OptionalReading {
        value: Option<f64>,
    }

    #[derive(crate::Message)]
    struct OptionalReadings {
        value: Vec<OptionalReading>,
    }

    #[test]
    fn optional_values_keep_presence_through_split_windows() {
        let columns = Columns::of::<OptionalReading>().unwrap();
        columns.check::<OptionalReadings>().unwrap();
        let expected = [None, Some(0.0), Some(2.5), None, Some(0.0), Some(-1.0)];
        let rows = expected.iter().map(|value| prost::Message::encode_to_vec(&OptionalReading { value: *value })).collect::<Vec<_>>();
        let encoded = columns.transpose(&rows).unwrap().encode_to_vec();
        let authored = <OptionalReadings as prost::Message>::decode(encoded.as_slice()).unwrap();
        assert_eq!(authored.value.into_iter().map(|row| row.value).collect::<Vec<_>>(), expected);
        let parts = parts(&columns, encoded, rows.len() as u64, 32).unwrap();
        assert!(parts.len() > 1);
        let mut actual = Vec::new();
        for part in parts {
            let Some(Frame::Batch(batch)) = part.frame else {
                panic!("batch")
            };
            let decoded = columns.decode(&batch.columns).unwrap();
            let Value::List(values) = decoded.get_field_by_number(1).unwrap().into_owned() else {
                panic!("column")
            };
            for value in values {
                let Value::Message(value) = value else {
                    panic!("optional element")
                };
                let field = value.descriptor().get_field(1).unwrap();
                actual.push(value.has_field(&field).then(|| value.get_field(&field).as_f64().unwrap()));
            }
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn transposed_rows_keep_every_default_value() {
        let columns = Columns::of::<Reading>().unwrap();
        let rows = [Reading { value: 0.0, label: "a".into() }, Reading { value: 2.5, label: String::new() }];
        let encoded = rows.iter().map(prost::Message::encode_to_vec).collect::<Vec<_>>();
        let transposed = columns.transpose(&encoded).unwrap().encode_to_vec();
        let decoded = <Readings as prost::Message>::decode(transposed.as_slice()).unwrap();
        assert_eq!(decoded.value, [0.0, 2.5]);
        assert_eq!(decoded.label, ["a", ""]);
        columns.check::<Readings>().unwrap();
    }

    #[test]
    fn parts_split_at_sample_boundaries_within_the_limit() {
        let columns = Columns::of::<Reading>().unwrap();
        let rows =
            (0..100).map(|index| prost::Message::encode_to_vec(&Reading { value: index as f64, label: "x".into() })).collect::<Vec<_>>();
        let encoded = columns.transpose(&rows).unwrap().encode_to_vec();
        let parts = parts(&columns, encoded, 100, 256).unwrap();
        assert!(parts.len() > 1);
        let mut next = 0;
        let mut values = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            assert!(part.encoded_len() <= 256);
            let Some(Frame::Batch(batch)) = &part.frame else {
                panic!("a batch part")
            };
            assert_eq!((batch.offset, batch.last), (next, index + 1 == parts.len()));
            next += batch.count;
            values.extend(<Readings as prost::Message>::decode(batch.columns.as_slice()).unwrap().value);
        }
        assert_eq!(values, (0..100).map(f64::from).collect::<Vec<_>>());
    }

    #[test]
    fn a_sample_larger_than_the_limit_names_the_action() {
        let columns = Columns::of::<Reading>().unwrap();
        let rows = [prost::Message::encode_to_vec(&Reading { value: 1.0, label: "x".repeat(64) })];
        let encoded = columns.transpose(&rows).unwrap().encode_to_vec();
        let error = parts(&columns, encoded, 1, 32).unwrap_err();
        assert_eq!(error.code(), tonic::Code::ResourceExhausted);
    }
}
