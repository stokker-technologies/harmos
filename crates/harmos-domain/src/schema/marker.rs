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

pub const PREFIX: &[u8] = b"\0HARMOS:ARTIFACT:V1:";

pub const FRAGMENT_LIMIT: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub const fn number<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: u64) {
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

pub const fn digits(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

pub const fn quoted_len(value: &str) -> usize {
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

pub const fn quoted<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: &str) {
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

pub const fn write<const N: usize>(output: &mut [u8; N], cursor: &mut usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        output[*cursor] = value[index];
        *cursor += 1;
        index += 1;
    }
}

/* === Private Functions === */

/* === Tests === */
