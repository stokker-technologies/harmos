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
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
use crate::__private::{decoding_add, decoding_vector};
use crate::MessageError;
use crate::schema::{Cardinality, Field, Kind, MAX_DEPTH, MAX_FIELDS, MAX_NAME_BYTES};
#[cfg(feature = "grpc")]
use crate::storage::codec::Unsigned;
use crate::storage::codec::Value;
#[cfg(feature = "grpc")]
use crate::storage::position::codec::{
    field_len, text_field, uint_field, write_bytes, write_header,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Result<T> = std::result::Result<T, MessageError>;

/* === Structs === */
pub(super) struct Fields;
struct Node<'a> {
    name: &'a str,
    tag: u32,
    kind: u64,
    cardinality: Cardinality,
    parent: usize,
    depth: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Value for Fields {
    fn postcard_write(fields: &Vec<Field>, bytes: &mut Vec<u8>) {
        use crate::__private::PostcardValue;
        let mut count = 0usize;
        visit(fields, 0, &mut 0, &mut |_| count += 1);
        count.write(bytes);
        visit(fields, 0, &mut 0, &mut |node| {
            node.name.len().write(bytes);
            bytes.extend_from_slice(node.name.as_bytes());
            (node.tag as u64).write(bytes);
            node.kind.write(bytes);
            cardinality(node.cardinality).write(bytes);
            (node.parent as u64).write(bytes);
        });
    }

    fn postcard_size(fields: &Vec<Field>) -> usize {
        use crate::__private::PostcardValue;
        let mut count = 0usize;
        let mut size = 0usize;
        visit(fields, 0, &mut 0, &mut |node| {
            count += 1;
            size = size
                .saturating_add(node.name.len().size())
                .saturating_add(node.name.len())
                .saturating_add((node.tag as u64).size())
                .saturating_add(node.kind.size())
                .saturating_add(cardinality(node.cardinality).size())
                .saturating_add((node.parent as u64).size());
        });
        size.saturating_add(count.size())
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        let mut count = 0usize;
        let mut names = 0usize;
        scan_postcard(input, &mut |node| {
            count += 1;
            if node.parent >= count {
                return Err(invalid());
            }
            names = decoding_add(names, node.name.len())?;
            Ok(())
        })?;
        memory(count, names)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Vec<Field>> {
        Self::postcard_memory(&mut input.clone())?;
        let count = input.clone().value::<usize>()?;
        let mut nodes = decoding_vector(count)?;
        scan_postcard(input, &mut |node| {
            nodes.push(node);
            Ok(())
        })?;
        assemble(nodes)
    }

    type Native = Vec<Field>;
    const KIND: Kind = Kind::Message {
        fields: Cow::Borrowed(&[
            Field {
                name: Cow::Borrowed("name"),
                tag: 1,
                kind: Kind::String,
                cardinality: Cardinality::Required,
            },
            Field {
                name: Cow::Borrowed("tag"),
                tag: 2,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: Cow::Borrowed("kind"),
                tag: 3,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: Cow::Borrowed("cardinality"),
                tag: 4,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: Cow::Borrowed("parent"),
                tag: 5,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            },
        ]),
    };
    const CARDINALITY: Cardinality = Cardinality::Repeated;
    #[cfg(feature = "grpc")]
    fn write(fields: &Vec<Field>, tag: u32, bytes: &mut Vec<u8>) {
        visit(fields, 0, &mut 0, &mut |node| {
            write_header(tag, node.size(), bytes);
            write_bytes(1, node.name.as_bytes(), bytes);
            Unsigned::write(&(node.tag as u64), 2, bytes);
            Unsigned::write(&node.kind, 3, bytes);
            Unsigned::write(&cardinality(node.cardinality), 4, bytes);
            Unsigned::write(&(node.parent as u64), 5, bytes);
        });
    }

    #[cfg(feature = "grpc")]
    fn size(fields: &Vec<Field>, tag: u32) -> usize {
        let mut size = 0usize;
        visit(fields, 0, &mut 0, &mut |node| {
            size = size
                .saturating_add(crate::__private::prost::encoding::key_len(tag))
                .saturating_add(field_len(node.size()).saturating_sub(1));
        });
        size
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let mut count = 0usize;
        let mut names = 0usize;
        scan(input, tag, &mut |node| {
            count += 1;
            if count > MAX_FIELDS || node.parent >= count {
                return Err(invalid());
            }
            names = decoding_add(names, node.name.len())?;
            Ok(())
        })?;
        memory(count, names)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<Field>> {
        Self::memory(input, tag)?;
        let mut count = 0usize;
        scan(input, tag, &mut |_| {
            count += 1;
            Ok(())
        })?;
        let mut nodes = decoding_vector(count)?;
        scan(input, tag, &mut |node| {
            nodes.push(node);
            Ok(())
        })?;
        assemble(nodes)
    }
}
#[cfg(feature = "grpc")]
impl Node<'_> {
    fn size(&self) -> usize {
        field_len(self.name.len())
            + Unsigned::size(&(self.tag as u64), 2)
            + Unsigned::size(&self.kind, 3)
            + Unsigned::size(&cardinality(self.cardinality), 4)
            + Unsigned::size(&(self.parent as u64), 5)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn memory(count: usize, names: usize) -> Result<usize> {
    // Borrowed Wire Nodes, Tree Assembly Slots, Native Fields And Validator
    // Scratch Can Overlap. Charge All Before Allocating Any Of Them.
    let slots = count.saturating_mul(
        std::mem::size_of::<Node<'_>>()
            + std::mem::size_of::<Field>()
            + std::mem::size_of::<Vec<Field>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<&str>()
            + std::mem::size_of::<u32>()
            + std::mem::size_of::<Vec<&str>>()
            + std::mem::size_of::<Vec<u32>>(),
    );
    decoding_add(
        decoding_add(names, slots)?,
        MAX_NAME_BYTES
            + std::mem::size_of::<crate::schema::Error>()
            + std::mem::size_of::<Vec<Field>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Vec<&str>>()
            + std::mem::size_of::<Vec<u32>>(),
    )
}

fn assemble(mut nodes: Vec<Node<'_>>) -> Result<Vec<Field>> {
    let count = nodes.len();
    let mut counts = vec![0usize; count + 1];
    for index in 0..count {
        let parent = nodes[index].parent;
        if parent > index {
            return Err(invalid());
        }
        if parent > 0 {
            if nodes[parent - 1].kind != 9 {
                return Err(invalid());
            }
            nodes[index].depth = nodes[parent - 1].depth + 1;
        }
        if nodes[index].depth > MAX_DEPTH {
            return Err(invalid());
        }
        counts[parent] += 1;
    }
    let mut children: Vec<Vec<Field>> = decoding_vector(count + 1)?;
    for count in counts {
        children.push(decoding_vector(count)?);
    }
    for (index, node) in nodes.into_iter().enumerate().rev() {
        let mut nested = std::mem::take(&mut children[index + 1]);
        nested.reverse();
        let kind = match node.kind {
            0 => Kind::Bool,
            1 => Kind::U32,
            2 => Kind::U64,
            3 => Kind::I32,
            4 => Kind::I64,
            5 => Kind::F32,
            6 => Kind::F64,
            7 => Kind::String,
            8 => Kind::Bytes,
            9 => Kind::Message {
                fields: Cow::Owned(nested),
            },
            _ => return Err(invalid()),
        };
        children[node.parent].push(Field {
            name: Cow::Owned(node.name.into()),
            tag: node.tag,
            kind,
            cardinality: node.cardinality,
        });
    }
    let mut fields = std::mem::take(&mut children[0]);
    fields.reverse();
    crate::schema::validate(&fields).map_err(|_| invalid())?;
    Ok(fields)
}

fn scan_postcard<'a>(
    input: &mut crate::__private::PostcardReader<'a>,
    visit: &mut impl FnMut(Node<'a>) -> Result<()>,
) -> Result<()> {
    input.nested(|input| {
        let count: usize = input.value()?;
        if count > MAX_FIELDS {
            return Err(invalid());
        }
        input.claim(count)?;
        for _ in 0..count {
            input.nested(|input| {
                let name: &str = input.value()?;
                if name.is_empty()
                    || name.len() > MAX_NAME_BYTES
                    || name.chars().any(char::is_control)
                {
                    return Err(invalid());
                }
                let tag = u32::try_from(input.value::<u64>()?).map_err(|_| invalid())?;
                if tag == 0 || tag > 536_870_911 || (19_000..=19_999).contains(&tag) {
                    return Err(invalid());
                }
                let kind: u64 = input.value()?;
                if kind > 9 {
                    return Err(invalid());
                }
                let cardinality = match input.value::<u64>()? {
                    0 => Cardinality::Required,
                    1 => Cardinality::Optional,
                    2 => Cardinality::Repeated,
                    3 => Cardinality::Variant,
                    _ => return Err(invalid()),
                };
                let parent = usize::try_from(input.value::<u64>()?).map_err(|_| invalid())?;
                visit(Node {
                    name,
                    tag,
                    kind,
                    cardinality,
                    parent,
                    depth: 0,
                })
            })?;
        }
        Ok(())
    })
}

pub(super) fn resident(fields: &Vec<Field>) -> usize {
    fields
        .capacity()
        .saturating_mul(std::mem::size_of::<Field>())
        + fields
            .iter()
            .map(|field| {
                let name = match &field.name {
                    Cow::Owned(name) => name.capacity(),
                    Cow::Borrowed(_) => 0,
                };
                name + match &field.kind {
                    Kind::Message {
                        fields: Cow::Owned(fields),
                    } => resident(fields),
                    _ => 0,
                }
            })
            .sum::<usize>()
}
#[cfg(feature = "grpc")]
fn scan<'a>(
    input: DecodeInput<'a>,
    tag: u32,
    visit: &mut impl FnMut(Node<'a>) -> Result<()>,
) -> Result<()> {
    input.fields(&mut |field| {
        if field.tag != tag {
            return Ok(());
        }
        let child = input.child(field.bytes()?)?;
        let name = text_field(child, 1)?.unwrap_or("");
        if name.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(invalid());
        }
        let tag = u32::try_from(uint_field(child, 2)?.unwrap_or(0)).map_err(|_| invalid())?;
        if tag == 0 || tag > 536_870_911 || (19_000..=19_999).contains(&tag) {
            return Err(invalid());
        }
        let kind = uint_field(child, 3)?.unwrap_or(0);
        if kind > 9 {
            return Err(invalid());
        }
        let cardinality = match uint_field(child, 4)?.unwrap_or(0) {
            0 => Cardinality::Required,
            1 => Cardinality::Optional,
            2 => Cardinality::Repeated,
            3 => Cardinality::Variant,
            _ => return Err(invalid()),
        };
        let parent = usize::try_from(uint_field(child, 5)?.unwrap_or(0)).map_err(|_| invalid())?;
        visit(Node {
            name,
            tag,
            kind,
            cardinality,
            parent,
            depth: 0,
        })
    })
}
fn visit(fields: &[Field], parent: usize, index: &mut usize, output: &mut impl FnMut(Node<'_>)) {
    for field in fields {
        *index += 1;
        let kind = match field.kind {
            Kind::Bool => 0,
            Kind::U32 => 1,
            Kind::U64 => 2,
            Kind::I32 => 3,
            Kind::I64 => 4,
            Kind::F32 => 5,
            Kind::F64 => 6,
            Kind::String => 7,
            Kind::Bytes => 8,
            Kind::Message { .. } => 9,
        };
        output(Node {
            name: &field.name,
            tag: field.tag,
            kind,
            cardinality: field.cardinality,
            parent,
            depth: 0,
        });
        if let Kind::Message { fields } = &field.kind {
            visit(fields, *index, index, output);
        }
    }
}
fn cardinality(value: Cardinality) -> u64 {
    match value {
        Cardinality::Required => 0,
        Cardinality::Optional => 1,
        Cardinality::Repeated => 2,
        Cardinality::Variant => 3,
    }
}
fn invalid() -> MessageError {
    MessageError::borrowed("invalid storage schema field tree")
}

/* === Tests === */
