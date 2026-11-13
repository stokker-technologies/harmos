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
use std::{net::SocketAddr, path::PathBuf, process::Stdio, time::Duration};

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

// Local Imports
use crate::{Error, Result, suite};

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
    let mut drain = None;
    let address = match target {
        Target::Address(address) => endpoint(&address)?,
        Target::Executable { path, arguments } => {
            let mut process =
                Command::new(path).args(arguments).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true).spawn()?;
            let stdout = process.stdout.take().ok_or_else(|| Error::Invalid("Missing Plugin Standard Output".into()))?;
            let mut lines = BufReader::new(stdout).lines();
            let announced = tokio::time::timeout(Duration::from_secs(30), async {
                while let Some(line) = lines.next_line().await? {
                    if let Some(address) = line.strip_prefix("harmos listening ") {
                        let socket: SocketAddr = address.parse().map_err(|_| Error::Invalid("Invalid Announced Address".into()))?;
                        if !socket.ip().is_loopback() || socket.port() == 0 {
                            return Err(Error::Invalid("Managed Address Must Be Loopback with a Bound Port".into()));
                        }
                        return Ok(address.to_owned());
                    }
                }
                Err(Error::Invalid("Plugin Exited Before Announcing Its Address".into()))
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
            drain = Some(tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} }));
            child = Some(process);
            let address = std::env::var("HARMOS_ADDRESS").unwrap_or(announced);
            endpoint(&address)?
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
    if let Some(drain) = drain {
        drain.abort();
        let _ = drain.await;
    }
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
