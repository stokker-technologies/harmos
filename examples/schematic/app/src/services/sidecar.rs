/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::path::Path;
use std::sync::Arc;

use crate::app::Runtime;
use crate::lanes::Lanes;
use harmos::config::PluginSidecarSettings;
use harmos::sidecar::Executable;
use harmos::{Error, Result};
use schematic_operations::Native;

pub const NAME: &str = "schematic_tools";

pub async fn start(runtime: &Runtime, path: &Path, directory: &Path) -> Result<()> {
    let program = std::fs::canonicalize(path).map_err(failed)?;
    let mut executable = Executable::at(&program).map_err(failed)?.directory(directory);
    if let Some(directory) = std::env::var_os("STKR_TEST_REPORT_DIR") {
        executable = executable.variable("STKR_TEST_REPORT_DIR", directory);
    }
    if executable.declaration().id != NAME {
        return Err(Error::NotReady { domain: "schematic plugin identity" });
    }
    let mut settings = PluginSidecarSettings::default();
    settings
        .plugin
        .host_operations
        .push(harmos::Identifier::declared(<harmos::builtins::streams::Publish as harmos::OperationDeclaration>::ID, "host operation"));
    runtime.plugin.register_sidecar::<Native>(executable, settings, Lanes::Open).await?;
    runtime.plugin.initialize().await
}

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}
