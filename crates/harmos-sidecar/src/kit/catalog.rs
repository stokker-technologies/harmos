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
use std::ffi::OsString;
use std::io::{self, Write};
// Third Party Imports

// Local Imports
use crate::{Plugin, Registrar, Registration, RegistrationError};

/* === Constants === */

pub const CATALOG_EXPORT_FLAG: &str = "--harmos-export-catalog-v1";

pub const CATALOG_EXPORT_LIMIT: usize = crate::Frame::LIMIT;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Bounded(Vec<u8>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > CATALOG_EXPORT_LIMIT - self.0.len() {
            return Err(io::Error::other("catalog export exceeds the byte limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn requested(mut args: impl Iterator<Item = OsString>) -> bool {
    args.next().is_some_and(|arg| arg == CATALOG_EXPORT_FLAG) && args.next().is_none()
}

pub(super) fn registrations<P: Plugin>() -> Result<Vec<Registration>, RegistrationError> {
    let mut registrar = Registrar::new();
    P::register(&mut registrar)?;
    Ok(registrar.finish())
}

pub(super) fn encode<P: Plugin>() -> io::Result<Vec<u8>> {
    let registrations = registrations::<P>().map_err(|error| io::Error::other(error.to_string()))?;
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, &registrations).map_err(io::Error::other)?;
    Ok(output.0)
}

/* === Private Functions === */

/* === Tests === */
