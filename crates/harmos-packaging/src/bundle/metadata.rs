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
use std::fmt;
use std::io::{self, Write};
// Third Party Imports
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
// Local Imports
use super::{Error, Executable, Identity, Limits, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Count {
    size: usize,
    limit: usize,
}

struct Files(Limits);

struct Registrations(usize);

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

impl<'de> Visitor<'de> for Files {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded file array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        #[derive(Deserialize)]
        struct File<'a> {
            #[serde(borrow)]
            executable: Option<&'a RawValue>,
        }
        #[derive(Deserialize)]
        struct Executable<'a> {
            #[serde(borrow)]
            registrations: &'a RawValue,
        }
        let mut count = 0usize;
        while let Some(raw) = sequence.next_element::<&'de RawValue>()? {
            if count >= self.0.files {
                return Err(serde::de::Error::custom("plugin file count limit exceeded"));
            }
            count += 1;
            let file: File<'_> = serde_json::from_str(raw.get()).map_err(serde::de::Error::custom)?;
            if let Some(raw) = file.executable {
                let executable: Executable<'_> = serde_json::from_str(raw.get()).map_err(serde::de::Error::custom)?;
                let mut json = serde_json::Deserializer::from_str(executable.registrations.get());
                json.deserialize_seq(Registrations(self.0.registrations)).map_err(serde::de::Error::custom)?;
                json.end().map_err(serde::de::Error::custom)?;
            }
        }
        Ok(())
    }
}

impl<'de> Visitor<'de> for Registrations {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded registration array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let mut count = 0usize;
        while sequence.next_element::<IgnoredAny>()?.is_some() {
            if count >= self.0 {
                return Err(serde::de::Error::custom("plugin registration count limit exceeded"));
            }
            count += 1;
        }
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

pub(super) fn preflight(bytes: &[u8], limits: Limits) -> Result<(), Error> {
    #[derive(Deserialize)]
    struct Manifest<'a> {
        #[serde(borrow)]
        files: &'a RawValue,
    }
    let manifest: Manifest<'_> = serde_json::from_slice(bytes)?;
    let mut json = serde_json::Deserializer::from_str(manifest.files.get());
    json.deserialize_seq(Files(limits))?;
    json.end()?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
