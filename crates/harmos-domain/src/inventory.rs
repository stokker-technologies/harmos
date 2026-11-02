/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod contracts;
pub mod contributions;
mod declaration;
pub mod listeners;
pub mod observers;
pub mod operations;
pub mod publications;

/* === Re-Exports === */

pub use contributions::Section;
pub use declaration::{CarrierSection, Declaration, DeclarationError, Facility, RestartPolicy};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::Deserialize;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDeclaration {
    pub metadata: crate::Metadata,

    #[serde(default)]
    pub row_metadata: Vec<crate::StreamField>,

    pub id: String,
    pub schema: u32,
    pub window: usize,
    pub row: crate::schema::graph::Schema,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) const fn number<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: u64) {
    let width = digits(value);
    let mut remaining = value;
    let mut position = width;
    while position > 0 {
        position -= 1;
        output[*cursor + position] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
    }
    *cursor += width;
}

pub(crate) const fn digits(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

pub(crate) const fn quoted_len(value: &str) -> usize {
    let mut len = 2;
    let mut index = 0;
    while index < value.len() {
        len += match value.as_bytes()[index] {
            b'"' | b'\\' => 2,
            0..=31 => 6,
            _ => 1,
        };
        index += 1;
    }
    len
}

pub(crate) const fn quoted<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: &str) {
    write(output, cursor, b"\"");
    let mut index = 0;
    while index < value.len() {
        let byte = value.as_bytes()[index];
        match byte {
            b'"' | b'\\' => {
                output[*cursor] = b'\\';
                *cursor += 1;
                output[*cursor] = byte;
                *cursor += 1;
            }
            0..=31 => {
                write(output, cursor, b"\\u00");
                output[*cursor] = b"0123456789abcdef"[(byte >> 4) as usize];
                output[*cursor + 1] = b"0123456789abcdef"[(byte & 15) as usize];
                *cursor += 2;
            }
            _ => {
                output[*cursor] = byte;
                *cursor += 1;
            }
        }
        index += 1;
    }
    write(output, cursor, b"\"");
}

pub(crate) const fn write<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        output[*cursor] = value[index];
        *cursor += 1;
        index += 1;
    }
}

/* === Private Functions === */

fn retain_schema(
    schema: crate::schema::graph::Schema, settings: &mut crate::config::SchemaSettings,
) -> Result<crate::schema::graph::Schema, crate::schema::graph::Error> {
    use crate::MemorySize;
    use crate::schema::graph::Error;
    let preparation = schema.preparation(settings)?;
    let bytes = schema.memory_size();
    settings.preparation_bytes =
        settings.preparation_bytes.get().checked_sub(bytes).and_then(std::num::NonZeroUsize::new).ok_or(Error::Allowance {
            budget: "memory bytes",
            used: bytes,
            allowed: settings.preparation_bytes.get(),
        })?;
    // Resolve Named Definitions And Validate The Result Under One Shared Work
    // Allowance Across Every Contract, Source And Publication In The Artifact.
    settings.preparation_work = preparation
        .work
        .checked_mul(2)
        .and_then(|work| settings.preparation_work.get().checked_sub(work))
        .and_then(std::num::NonZeroUsize::new)
        .ok_or(Error::Allowance {
            budget: "work",
            used: preparation.work.saturating_mul(2),
            allowed: settings.preparation_work.get(),
        })?;
    Ok(schema)
}

/* === Tests === */
