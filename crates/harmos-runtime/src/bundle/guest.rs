/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read Guest Registration Metadata

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{Error, invalid};
use prost::Message as _;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const METADATA_SECTION: &str = "harmos.v1.GuestRegistration";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn registration(component: &[u8]) -> Result<harmos_protocol::v1::GuestRegistration, Error> {
    let mut bytes = component.strip_prefix(b"\0asm\x0d\0\x01\0").ok_or_else(|| invalid("guest executable is not a component"))?;
    let mut registration = None;
    while let Some((&section, rest)) = bytes.split_first() {
        bytes = rest;
        let data = delimited(&mut bytes)?;
        if section != 0 {
            continue;
        }
        let mut custom = data;
        let name = delimited(&mut custom)?;
        if name != METADATA_SECTION.as_bytes() {
            continue;
        }
        if registration.is_some() {
            return Err(invalid("duplicate guest registration metadata"));
        }
        if custom.len() > harmos_protocol::guest::COMMAND_BYTES {
            return Err(Error::Limit("guest registration bytes"));
        }
        let value = harmos_protocol::v1::GuestRegistration::decode(custom).map_err(|error| invalid(error.to_string()))?;
        let declarations = value.declarations.as_ref().ok_or_else(|| invalid("guest declarations is missing"))?;
        if declarations.digest != harmos_protocol::schema::declarations_digest(declarations) {
            return Err(invalid("guest declarations digest differs"));
        }
        registration = Some(value);
    }
    registration.ok_or_else(|| invalid("guest registration metadata is missing"))
}

/* === Private Functions === */

fn delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let length = prost::encoding::decode_varint(bytes).map_err(|error| invalid(error.to_string()))?;
    let length = usize::try_from(length).map_err(|error| invalid(error.to_string()))?;
    if length > bytes.len() {
        return Err(invalid("truncated guest component section"));
    }
    let (data, rest) = bytes.split_at(length);
    *bytes = rest;
    Ok(data)
}

/* === Tests === */
