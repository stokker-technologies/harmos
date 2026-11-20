/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Process Readiness, Plain gRPC Dialing and Case Reporting.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{path::PathBuf, process::Stdio, time::Duration};

// Third Party Imports
use tokio::process::{Child, Command};

// Local Imports
use crate::{Error, Result, suite};
use harmos_runtime::plugin::grpc::Launches;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Target {
    Address(String),
    Executable { path: PathBuf, arguments: Vec<String> },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn run(target: Target) -> Result<bool> {
    let mut child = None;
    // The Launch Lives Until the Cases End, So Its Listener Outlives the Plugin's Announcement.
    let launches = Launches::new();
    let address = match target {
        Target::Address(address) => endpoint(&address)?,
        Target::Executable { path, arguments } => {
            let mut launch = launches.reserve(path.display().to_string()).await.map_err(|error| Error::Invalid(error.to_string()))?;
            // Standard Output Would Mix With the Case Table; Standard Error Stays Visible.
            let mut process = Command::new(path)
                .args(arguments)
                .envs(launch.environment())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()?;
            let announced = tokio::time::timeout(Duration::from_secs(30), async {
                tokio::select! {
                    biased;
                    exit = process.wait() => Err(Error::Invalid(format!("Plugin Exited With {} Before Announcing Its Address", exit?))),
                    announced = launch.announced() => announced.map_err(|error| Error::Invalid(error.to_string())),
                }
            })
            .await;
            let announced = match announced {
                Ok(Ok(address)) => address,
                error => {
                    let _ = process.kill().await;
                    return Err(match error {
                        Ok(Err(error)) => error,
                        _ => Error::Invalid("Plugin Readiness Hang Guard Expired".into()),
                    });
                }
            };
            child = Some(process);
            endpoint(&announced.to_string())?
        }
    };
    let mut cases = suite::run(&address, child.is_some()).await?;
    if let Some(mut process) = child {
        let result = finish(&mut process).await;
        if let Err(error) = result {
            let stop = cases.last_mut().expect("Stop Case Exists");
            let previous = stop.error.take().map(|error| format!("{error}; ")).unwrap_or_default();
            stop.error = Some(format!("{previous}{error}"));
        }
    }
    launches.shutdown().await;
    println!("| Rule | Case | Result |");
    println!("| --- | --- | --- |");
    for case in &cases {
        let result = case
            .error
            .as_ref()
            .map(|error| format!("FAIL: {}", error.replace('|', "/").replace(['\n', '\r'], " ")))
            .unwrap_or_else(|| "PASS".into());
        println!("| {:02} | {} | {result} |", case.rule, case.name);
    }
    let passed = cases.iter().filter(|case| case.error.is_none()).count();
    println!("{passed}/{} Cases Passed", cases.len());
    Ok(passed == cases.len())
}

/* === Private Functions === */

fn endpoint(address: &str) -> Result<String> {
    if address.starts_with("https://") || address.contains('@') {
        return Err(Error::Invalid("Only Plain gRPC Without Credentials Is Supported".into()));
    }
    Ok(if address.starts_with("http://") { address.into() } else { format!("http://{address}") })
}

async fn finish(child: &mut Child) -> Result<()> {
    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(Error::Invalid(format!("Managed Plugin Exited with {status}"))),
        Ok(Err(error)) => Err(error.into()),
        Err(_) => {
            child.kill().await?;
            Err(Error::Invalid("Managed Plugin Did Not Exit After Stop".into()))
        }
    }
}

/* === Tests === */
