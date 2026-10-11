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
use super::{Cardinality, Field, Kind};
use crate::inventory::{digits, number, quoted, quoted_len, write};

/* === Constants === */
const NAME: &[u8] = b"{\"name\":";
const TAG: &[u8] = b",\"tag\":";
const KIND: &[u8] = b",\"kind\":{\"type\":";
const NESTED: &[u8] = b",\"fields\":";
const CARDINALITY: &[u8] = b"},\"cardinality\":";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(crate) const fn encoded_len(fields: &[Field]) -> usize {
    let mut length = 2;
    let mut index = 0;
    while index < fields.len() {
        let field = &fields[index];
        length += NAME.len()
            + quoted_len(name(field))
            + TAG.len()
            + digits(field.tag as u64)
            + KIND.len()
            + quoted_len(kind(&field.kind))
            + CARDINALITY.len()
            + quoted_len(cardinality(field.cardinality))
            + 1;
        if let Kind::Message { fields } = &field.kind {
            length += NESTED.len() + encoded_len(borrowed(fields));
        }
        if index > 0 {
            length += 1;
        }
        index += 1;
    }
    length
}

pub(crate) const fn encode_into<const N: usize>(
    output: &mut [u8; N],
    cursor: &mut usize,
    fields: &[Field],
) {
    write(output, cursor, b"[");
    let mut index = 0;
    while index < fields.len() {
        let field = &fields[index];
        if index > 0 {
            write(output, cursor, b",");
        }
        write(output, cursor, NAME);
        quoted(output, cursor, name(field));
        write(output, cursor, TAG);
        number(output, cursor, field.tag as u64);
        write(output, cursor, KIND);
        quoted(output, cursor, kind(&field.kind));
        if let Kind::Message { fields } = &field.kind {
            write(output, cursor, NESTED);
            encode_into(output, cursor, borrowed(fields));
        }
        write(output, cursor, CARDINALITY);
        quoted(output, cursor, cardinality(field.cardinality));
        write(output, cursor, b"}");
        index += 1;
    }
    write(output, cursor, b"]");
}

/* === Private Functions === */
const fn name(field: &Field) -> &str {
    match &field.name {
        Cow::Borrowed(name) => name,
        Cow::Owned(_) => panic!("constant schema names must be borrowed"),
    }
}

// Const encoding must inspect ownership; Cow's slice dereference is not const.
#[allow(clippy::ptr_arg)]
const fn borrowed<'a>(fields: &'a Cow<'static, [Field]>) -> &'a [Field] {
    match fields {
        Cow::Borrowed(fields) => fields,
        Cow::Owned(_) => panic!("constant nested fields must be borrowed"),
    }
}

const fn kind(kind: &Kind) -> &'static str {
    match kind {
        Kind::Bool => "bool",
        Kind::U32 => "u32",
        Kind::U64 => "u64",
        Kind::I32 => "i32",
        Kind::I64 => "i64",
        Kind::F32 => "f32",
        Kind::F64 => "f64",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Message { .. } => "message",
    }
}

const fn cardinality(cardinality: Cardinality) -> &'static str {
    match cardinality {
        Cardinality::Required => "required",
        Cardinality::Optional => "optional",
        Cardinality::Repeated => "repeated",
        Cardinality::Variant => "variant",
    }
}

/* === Tests === */
