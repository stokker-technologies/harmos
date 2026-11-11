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
use std::path::Path;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::app::Runtime;
use crate::lanes::Lanes;
use harmos::{OperationRequest, encoding::Message as _};
use harmos_runtime::plugin::grpc::{Attached, Managed, Placement, PlacementSettings};
use harmos_runtime::{Error, Result};

/* === Constants === */

pub const NAME: &str = "schematic_tools";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn start(runtime: &Runtime, path: &Path, directory: &Path) -> Result<Attached> {
    let executable = std::fs::canonicalize(path).map_err(failed)?;
    let directory = std::fs::canonicalize(directory).map_err(failed)?;
    let mut environment = std::collections::BTreeMap::from([("SCHEMATIC_DIRECTORY".into(), directory.into_os_string())]);
    if let Some(directory) = std::env::var_os("STKR_TEST_REPORT_DIR") {
        environment.insert("STKR_TEST_REPORT_DIR".into(), directory);
    }

    // Keep Operation Streaming in the Host
    let resources = runtime.operations.context().resource::<schematic_operations::schematic::upload::AnalysisResources>()?;
    let streams = runtime.streams.clone();
    resources.progress.get_or_init(|| Arc::new(move |row| streams.publish(row)));
    let registration = harmos_runtime::operations::Registration::new(Lanes::Open)
        .operation::<schematic_operations::AnalyzeSchematic>("schematic.analyze")?
        .operation::<schematic_operations::AnalyzeUpload>("schematic.analyze.upload")?
        .operation::<schematic_operations::AnalyzeExchange>("schematic.analyze.exchange")?
        .operation::<schematic_operations::transfer::Upload>("benchmark.upload")?
        .operation::<schematic_operations::transfer::Download>("benchmark.download")?
        .operation::<schematic_operations::transfer::Put>("benchmark.put")?
        .operation::<schematic_operations::transfer::Take>("benchmark.take")?;
    runtime.register_batch(registration).await?;

    // Attach the Unary Plugin
    let placement = Placement::Managed(Managed {
        executable,
        arguments: Vec::new(),
        environment,
    });
    let settings = PlacementSettings {
        connect_timeout: std::time::Duration::from_secs(10),
        reconnect_delay: std::time::Duration::from_millis(100),
        stop_grace: std::time::Duration::from_secs(10),
        stream_window: std::num::NonZeroUsize::new(16).unwrap(),
    };
    let attached = runtime.attach_grpc(placement, settings).await.map_err(failed)?;
    let activation = attached.activation().ok_or(Error::NotReady { domain: "schematic plugin activation" })?;
    if activation.description.id != NAME {
        attached.stop("Unexpected Plugin Identity").await.map_err(failed)?;
        return Err(Error::NotReady { domain: "schematic plugin identity" });
    }
    Ok(attached)
}

pub async fn call<Q: OperationRequest>(runtime: &Runtime, request: Q) -> Result<Q::Response> {
    let payload = request.encode_to_vec();
    let response = runtime.operations.call_encoded(Q::ID, payload).await?;
    Q::Response::decode(response.as_slice()).map_err(failed)
}

/* === Private Functions === */

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
