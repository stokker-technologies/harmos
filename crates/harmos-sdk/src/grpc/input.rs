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

// Third Party Imports

// Local Imports
use harmos_domain::__private::decoding_add as add;
use harmos_domain::MessageError;

/* === Constants === */
const DEPTH: usize = 32;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct DecodeInput<'a> {
    source: Source<'a>,
    depth: usize,
}

#[derive(Clone, Copy)]
pub struct DecodeField<'a> {
    pub tag: u32,
    pub value: DecodeValue<'a>,
}

/* === Enums === */
#[derive(Clone, Copy)]
enum Source<'a> {
    Bytes(&'a [u8]),
    Fields {
        parent: &'a DecodeInput<'a>,
        tag: u32,
        after: usize,
    },
}

#[derive(Clone, Copy)]
pub enum DecodeValue<'a> {
    Varint(u64),
    Bits32(u32),
    Bits64(u64),
    Bytes(&'a [u8]),
    Group,
}

#[derive(Clone, Copy)]
pub enum DecodeScalar {
    Varint,
    Bits32,
    Bits64,
}

/* === Traits === */

/* === Implementations === */
impl<'a> DecodeInput<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            source: Source::Bytes(bytes),
            depth: 0,
        }
    }

    pub fn child(&self, bytes: &'a [u8]) -> Result<Self, MessageError> {
        self.at(Source::Bytes(bytes))
    }

    pub fn field(&self, tag: u32) -> Result<DecodeInput<'_>, MessageError> {
        self.at(Source::Fields {
            parent: self,
            tag,
            after: 0,
        })
    }

    // Selects The Last Declared Variant; None Only When No Variant Appears.
    // Every variant is a length-delimited message field, so an undeclared
    // length-delimited tag is a variant this build does not know and refuses
    // the whole body. Other wire types can never be variants and stay ignored.
    pub fn variant(&self, tags: &[u32]) -> Result<Option<(u32, DecodeInput<'_>)>, MessageError> {
        let mut selected = None;
        let mut after = 0;
        let mut ordinal = 0usize;
        self.fields(&mut |field| {
            if tags.contains(&field.tag) {
                self.child(field.bytes()?)?.fields(&mut |_| Ok(()))?;
                if selected != Some(field.tag) {
                    selected = Some(field.tag);
                    after = ordinal;
                }
            } else if matches!(field.value, DecodeValue::Bytes(_)) {
                return Err(MessageError::borrowed("unknown protobuf enum variant"));
            }
            ordinal = add(ordinal, 1)?;
            Ok(())
        })?;
        let Some(tag) = selected else {
            return Ok(None);
        };
        let input = self.at(Source::Fields {
            parent: self,
            tag,
            after,
        })?;
        Ok(Some((tag, input)))
    }

    pub fn contiguous(&self) -> Result<&'a [u8], MessageError> {
        let mut part = None;
        self.parts(&mut |bytes| {
            if part.is_some() {
                return Err(MessageError::borrowed(
                    "merged decoding memory bound is unsupported",
                ));
            }
            part = Some(bytes);
            Ok(())
        })?;
        Ok(part.unwrap_or(&[]))
    }

    pub fn fields(
        &self,
        visit: &mut dyn FnMut(DecodeField<'a>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        self.parts(&mut |mut bytes| {
            while !bytes.is_empty() {
                visit(read(&mut bytes, self.depth)?)?;
            }
            Ok(())
        })
    }

    fn at<'b>(&self, source: Source<'b>) -> Result<DecodeInput<'b>, MessageError> {
        let depth = add(self.depth, 1)?;
        if depth > DEPTH {
            return Err(MessageError::borrowed(
                "protobuf decode exceeds 32 nested levels",
            ));
        }
        Ok(DecodeInput { source, depth })
    }

    fn parts(
        &self,
        visit: &mut dyn FnMut(&'a [u8]) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        match self.source {
            Source::Bytes(bytes) => visit(bytes),
            Source::Fields { parent, tag, after } => {
                let mut ordinal = 0usize;
                parent.fields(&mut |field| {
                    if ordinal >= after && field.tag == tag {
                        visit(field.bytes()?)?;
                    }
                    ordinal = add(ordinal, 1)?;
                    Ok(())
                })
            }
        }
    }
}

impl<'a> DecodeField<'a> {
    pub fn bytes(self) -> Result<&'a [u8], MessageError> {
        match self.value {
            DecodeValue::Bytes(bytes) => Ok(bytes),
            _ => Err(MessageError::borrowed(
                "expected length-delimited protobuf field",
            )),
        }
    }

    pub fn text(self) -> Result<&'a str, MessageError> {
        std::str::from_utf8(self.bytes()?)
            .map_err(|_| MessageError::borrowed("invalid protobuf string UTF-8"))
    }

    pub fn scalar(self, kind: DecodeScalar) -> Result<DecodeValue<'a>, MessageError> {
        match (kind, self.value) {
            (DecodeScalar::Varint, DecodeValue::Varint(_))
            | (DecodeScalar::Bits32, DecodeValue::Bits32(_))
            | (DecodeScalar::Bits64, DecodeValue::Bits64(_)) => Ok(self.value),
            _ => Err(MessageError::borrowed("protobuf scalar wire type mismatch")),
        }
    }

    pub fn scalars(
        self,
        kind: DecodeScalar,
        visit: &mut dyn FnMut(DecodeValue<'a>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        let DecodeValue::Bytes(mut bytes) = self.value else {
            return visit(self.scalar(kind)?);
        };
        while !bytes.is_empty() {
            let value = match kind {
                DecodeScalar::Varint => DecodeValue::Varint(varint(&mut bytes)?),
                DecodeScalar::Bits32 => DecodeValue::Bits32(u32::from_le_bytes(
                    take(&mut bytes, 4)?.try_into().expect("four bytes"),
                )),
                DecodeScalar::Bits64 => DecodeValue::Bits64(u64::from_le_bytes(
                    take(&mut bytes, 8)?.try_into().expect("eight bytes"),
                )),
            };
            visit(value)?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn varint(bytes: &mut &[u8]) -> Result<u64, MessageError> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *take(bytes, 1)?.first().expect("one byte");
        if shift == 63 && byte > 1 {
            return Err(MessageError::borrowed("invalid protobuf varint"));
        }
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    Err(MessageError::borrowed("invalid protobuf varint"))
}

fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8], MessageError> {
    if length > bytes.len() {
        return Err(MessageError::borrowed("truncated protobuf field"));
    }
    let (value, rest) = bytes.split_at(length);
    *bytes = rest;
    Ok(value)
}

fn key(bytes: &mut &[u8]) -> Result<(u32, u8), MessageError> {
    let key = varint(bytes)?;
    let tag =
        u32::try_from(key >> 3).map_err(|_| MessageError::borrowed("invalid protobuf tag"))?;
    if tag == 0 || tag > 536_870_911 {
        return Err(MessageError::borrowed("invalid protobuf tag"));
    }
    Ok((tag, (key & 7) as u8))
}

fn read<'a>(bytes: &mut &'a [u8], depth: usize) -> Result<DecodeField<'a>, MessageError> {
    let (tag, wire) = key(bytes)?;
    let value = value(tag, wire, bytes, depth)?;
    Ok(DecodeField { tag, value })
}

fn value<'a>(
    tag: u32,
    wire: u8,
    bytes: &mut &'a [u8],
    depth: usize,
) -> Result<DecodeValue<'a>, MessageError> {
    Ok(match wire {
        0 => DecodeValue::Varint(varint(bytes)?),
        1 => DecodeValue::Bits64(u64::from_le_bytes(
            take(bytes, 8)?.try_into().expect("eight bytes"),
        )),
        2 => {
            let length = usize::try_from(varint(bytes)?)
                .map_err(|_| MessageError::borrowed("protobuf length exceeds address space"))?;
            DecodeValue::Bytes(take(bytes, length)?)
        }
        5 => DecodeValue::Bits32(u32::from_le_bytes(
            take(bytes, 4)?.try_into().expect("four bytes"),
        )),
        3 => {
            if depth >= DEPTH {
                return Err(MessageError::borrowed(
                    "protobuf decode exceeds 32 nested levels",
                ));
            }
            loop {
                let (nested, wire) = key(bytes)?;
                if wire == 4 {
                    if nested != tag {
                        return Err(MessageError::borrowed("mismatched protobuf group"));
                    }
                    break;
                }
                value(nested, wire, bytes, depth + 1)?;
            }
            DecodeValue::Group
        }
        _ => return Err(MessageError::borrowed("invalid protobuf wire type")),
    })
}

/* === Tests === */
