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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub id: String,
    pub version: semver::VersionReq,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub const fn artifact_subset(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }

    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'-' | b'.' | b'_'))
        {
            return false;
        }
        index += 1;
    }

    true
}

pub const fn qualified_len(owner: &str, name: &str) -> usize {
    owner.len() + 1 + name.len()
}

pub const fn qualify<const N: usize>(owner: &str, name: &str) -> [u8; N] {
    assert!(N == qualified_len(owner, name));
    let mut output = [0; N];
    let mut cursor = 0;
    let owner = owner.as_bytes();
    let mut index = 0;
    while index < owner.len() {
        output[cursor] = owner[index];
        cursor += 1;
        index += 1;
    }
    output[cursor] = b'.';
    cursor += 1;
    let name = name.as_bytes();
    let mut index = 0;
    while index < name.len() {
        output[cursor] = name[index];
        cursor += 1;
        index += 1;
    }

    output
}

pub const fn qualified(bytes: &'static [u8]) -> &'static str {
    match core::str::from_utf8(bytes) {
        Ok(id) => id,
        Err(_) => panic!("a qualified id is built from two portable names"),
    }
}

pub fn stream_subset(name: &str) -> bool {
    name.split('.').all(|segment| {
        let mut characters = segment.chars();
        characters
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && characters.all(|character| {
                character == '_' || character == '-' || character.is_ascii_alphanumeric()
            })
    })
}

/* === Private Functions === */

/* === Tests === */
