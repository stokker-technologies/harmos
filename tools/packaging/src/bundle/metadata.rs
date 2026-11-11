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
use std::io::{self, Write};
// Third Party Imports
use serde::Serialize;
// Local Imports
use super::{Error, Executable, Identity, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Count {
    size: usize,
    limit: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.size) {
            return Err(io::Error::other("plugin manifest byte limit exceeded"));
        }
        self.size += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn identity_size(identity: &Identity, limit: usize) -> Result<usize, Error> {
    let mut output = Count { size: 0, limit };
    output.write_all(b"{\"schema\":1,\"identity\":")?;
    serde_json::to_writer(&mut output, identity)?;
    output.write_all(b",\"files\":[]}")?;
    Ok(output.size)
}

pub(super) fn file_size(source: &Source<'_>, sha256: &str, comma: bool, size: &mut usize, limit: usize) -> Result<(), Error> {
    // Same fields/order as File, borrowing the export instead of cloning it.
    #[derive(Serialize)]
    struct File<'a> {
        path: &'a str,
        size: u64,
        sha256: &'a str,

        #[serde(skip_serializing_if = "Option::is_none")]
        executable: Option<&'a Executable>,
    }
    let mut output = Count { size: *size, limit };
    if comma {
        output.write_all(b",")?;
    }
    serde_json::to_writer(
        &mut output,
        &File {
            path: source.path,
            size: source.bytes.len() as u64,
            sha256,
            executable: source.executable.as_ref(),
        },
    )?;
    *size = output.size;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
