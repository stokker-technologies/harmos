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
use std::{io, path::PathBuf};
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Result<T, E = Error> = std::result::Result<T, E>;

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot {operation} {path}: {source}")]
    Publication {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/* === Traits === */

/* === Implementations === */
impl Error {
    pub(crate) fn publication(
        operation: &'static str,
        path: &std::path::Path,
        source: io::Error,
    ) -> Self {
        Self::Publication {
            operation,
            path: path.to_owned(),
            source,
        }
    }

    pub fn kind(&self) -> io::ErrorKind {
        let Self::Publication { source, .. } = self;
        source.kind()
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        let Error::Publication { source, .. } = error;
        source
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
