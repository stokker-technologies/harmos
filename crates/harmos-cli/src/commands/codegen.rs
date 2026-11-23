/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate SDK Bindings From Descriptors.

/* === Modules === */

mod cpp;
mod csharp;
mod python;
mod rust;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// Third Party Imports
use clap::Subcommand;

// Local Imports
use crate::descriptor::{Face, Method, Schema};
use crate::error::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Inputs every language shares.
#[derive(clap::Args)]
pub(crate) struct Inputs {
    /// Descriptor sets compiled with their imports; several merge by file name.
    #[arg(long = "descriptors", required = true)]
    descriptors: Vec<PathBuf>,

    /// Directory that receives the generated files.
    #[arg(long)]
    out: PathBuf,

    /// Proto namespace whose SDK the bindings belong to.
    #[arg(long, default_value = "harmos")]
    namespace: String,

    /// Generate an application's client and plugin context over the published Harmos SDK instead of SDK internals.
    #[arg(long)]
    application: bool,

    /// Keep this service's operation identities but omit its host resources, as `package.Service` (repeatable).
    #[arg(long = "exclude-service", value_name = "SERVICE")]
    excluded_services: Vec<String>,
}

/// A loaded descriptor inventory and the calls each face renders.
pub(crate) struct Generation {
    pub(crate) namespace: String,
    pub(crate) application: bool,
    pub(crate) schema: Schema,
    pub(crate) library: Schema,
    pub(crate) clients: Vec<Method>,
    pub(crate) contexts: Vec<Method>,
}

/// Rendered files relative to the output directory.
#[derive(Default)]
pub(crate) struct Output {
    files: Vec<(PathBuf, String)>,
}

/* === Enums === */

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Rust SDK exports, client resources and host resources.
    Rust(rust::Args),

    /// Python wire modules, typed client resources and host calls.
    Python(python::Args),

    /// C# wire messages, typed client resources and host calls.
    Csharp(csharp::Args),

    /// C++ payload exports, typed client resources and host calls.
    Cpp(cpp::Args),
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Self::Rust(args) => rust::run(args),
            Self::Python(args) => python::run(args),
            Self::Csharp(args) => csharp::run(args),
            Self::Cpp(args) => cpp::run(args),
        }
    }
}

impl Inputs {
    pub(crate) fn out(&self) -> &Path {
        &self.out
    }

    /// Read the descriptors and split the method table by face.
    pub(crate) fn load(&self) -> Result<Generation> {
        let identifier = self.namespace.chars().next().is_some_and(|first| first.is_ascii_lowercase())
            && self.namespace.chars().all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_');
        if !identifier {
            return Err(Error::Input("Generate SDK: Supply One Lowercase Package Name in --namespace".into()));
        }
        let schema = Schema::load(&self.descriptors)?;
        let library = Schema::library()?;
        let methods = schema.table()?;
        let excluded: BTreeSet<&str> = self.excluded_services.iter().map(String::as_str).collect();
        let clients = methods.iter().filter(|method| method.face == Face::Client).cloned().collect();
        let contexts = methods
            .into_iter()
            .filter(|method| method.face == Face::Context)
            .map(|mut method| {
                if excluded.contains(format!("{}.{}", method.package, method.service).as_str()) {
                    method.surface = false;
                    method.fields.clear();
                    method.payload_field = None;
                }
                method
            })
            .collect();
        Ok(Generation {
            namespace: self.namespace.clone(),
            application: self.application,
            schema,
            library,
            clients,
            contexts,
        })
    }
}

impl Output {
    pub(crate) fn add(&mut self, path: impl Into<PathBuf>, lines: &[String]) {
        let mut source = lines.join("\n");
        source.push('\n');
        self.files.push((path.into(), source));
    }

    pub(crate) fn add_source(&mut self, path: impl Into<PathBuf>, source: String) {
        self.files.push((path.into(), source));
    }

    /// Write every file below `out`, creating folders as needed.
    pub(crate) fn write(self, out: &Path) -> Result<()> {
        for (relative, source) in self.files {
            let path = out.join(relative);
            let folder = path.parent().expect("generated files live below the output directory");
            fs::create_dir_all(folder).map_err(|source| Error::Io {
                action: "Create Output Folder",
                path: folder.to_path_buf(),
                source,
            })?;
            fs::write(&path, source).map_err(|source| Error::Io {
                action: "Write Generated File",
                path: path.clone(),
                source,
            })?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/// Replace `word` wherever no identifier character follows it and `joined` rejects the character before it.
pub(crate) fn replace_word(source: &str, word: &str, replacement: &str, joined: impl Fn(char) -> bool) -> String {
    let mut result = String::with_capacity(source.len());
    let mut copied = 0;
    let mut search = 0;
    while let Some(offset) = source[search..].find(word) {
        let start = search + offset;
        let end = start + word.len();
        let before = source[..start].chars().next_back().is_some_and(&joined);
        let after = source[end..].chars().next().is_some_and(identifier_character);
        if before || after {
            search = start + word.chars().next().map_or(1, char::len_utf8);
            continue;
        }
        result.push_str(&source[copied..start]);
        result.push_str(replacement);
        copied = end;
        search = end;
    }
    result.push_str(&source[copied..]);
    result
}

/// A character of a Rust, C#, C++ or Python identifier.
pub(crate) fn identifier_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Map an I/O error onto the failed action and its path.
pub(crate) fn failure(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Error {
    move |source| Error::Io { action, path: path.to_path_buf(), source }
}

/// One `--name=value` style argument without lossy path conversion.
pub(crate) fn flag(name: &str, value: impl AsRef<OsStr>) -> OsString {
    let mut argument = OsString::from(name);
    argument.push(value);
    argument
}

/* === Private Functions === */

/* === Tests === */
