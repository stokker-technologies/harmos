/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Native compiler tools and sidecar catalog capture for authoring tools.

use crate::{Artifact, BuildCancellation, Error, ProcessSettings};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn host_target(directory: Option<&Path>, settings: ProcessSettings, cancellation: &BuildCancellation) -> Result<String, Error> {
    let mut command = Command::new("rustc");
    command.arg("-vV");
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = crate::process::capture(command, settings, cancellation)?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| Error::Cargo("rustc did not report its host target".into()))
}

/// Write a separate shipping copy while preserving the compiler's original.
pub fn strip_debug(
    path: &Path, directory: &Path, target: &str, settings: ProcessSettings, cancellation: &BuildCancellation,
) -> Result<PathBuf, Error> {
    let mut command = Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process::capture(command, settings, cancellation)?;
    let objcopy = PathBuf::from(String::from_utf8_lossy(&sysroot.stdout).trim())
        .join("lib/rustlib")
        .join(target)
        .join("bin")
        .join(format!("llvm-objcopy{}", std::env::consts::EXE_SUFFIX));
    if !objcopy.is_file() {
        return Err(Error::Settings(format!("{} is missing; install the llvm-tools component", objcopy.display())));
    }
    let name = path.file_name().ok_or_else(|| Error::Settings("executable has no file name".into()))?;
    std::fs::create_dir_all(directory)?;
    let output = directory.join(name);
    if std::path::absolute(path)? == std::path::absolute(&output)? {
        return Err(Error::Settings("shipping copy must preserve the compiler output".into()));
    }
    let mut command = Command::new(objcopy);
    command.arg("--strip-debug").arg(path).arg(&output);
    crate::process::capture(command, settings, cancellation)?;
    Ok(output)
}

#[derive(Debug)]
pub struct ExportedCatalog {
    pub registrations: Vec<harmos_domain::Registration>,
    pub output_bytes: usize,
}

/// Run only the stamped executable and refuse changes while it exports.
pub fn catalog(
    artifact: &Artifact, directory: Option<&Path>, settings: ProcessSettings, cancellation: &BuildCancellation,
) -> Result<ExportedCatalog, Error> {
    let verify = || {
        let current = Artifact::read(artifact.producer.clone(), artifact.name.clone(), artifact.path.clone())?;
        if current.sha256 != artifact.sha256 {
            return Err(Error::ArtifactChanged(artifact.path.clone()));
        }
        Ok(())
    };
    verify()?;
    let mut command = Command::new(std::path::absolute(&artifact.path)?);
    command.arg(harmos_domain::CATALOG_EXPORT_FLAG);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = crate::process::capture(command, settings, cancellation)?;
    verify()?;
    Ok(ExportedCatalog {
        registrations: serde_json::from_slice(&output.stdout)?,
        output_bytes: output.stdout.len(),
    })
}
