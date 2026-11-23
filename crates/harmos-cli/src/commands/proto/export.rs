/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Write the Authoritative Protocol Sources.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs;
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use crate::error::{Error, Result};

/* === Constants === */

// The Build Script Lists Every Bundled `proto/harmos` File.
include!(concat!(env!("OUT_DIR"), "/protocols.rs"));

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Directory that receives `harmos/**/*.proto`; use it as a protoc import root.
    #[arg(long)]
    out: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    for (name, source) in PROTOCOLS {
        let path = args.out.join(name);
        let folder = path.parent().expect("bundled protocols live below their namespace");
        fs::create_dir_all(folder).map_err(|source| Error::Io {
            action: "Create Protocol Folder",
            path: folder.to_path_buf(),
            source,
        })?;
        fs::write(&path, source).map_err(|source| Error::Io {
            action: "Write Protocol",
            path: path.clone(),
            source,
        })?;
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
