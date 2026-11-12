/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Plugin Process Ownership

/* === Modules === */

#[cfg(target_os = "windows")]
mod windows;
#[cfg(unix)]
mod unix;

/* === Re-Exports === */

#[cfg(target_os = "windows")]
use windows::command;
#[cfg(unix)]
use unix::command;

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    ffi::OsString,
    net::SocketAddr,
    path::PathBuf,
    process::{ExitStatus, Stdio},
};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
    task::JoinHandle,
};

// Local Imports
use super::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Native Executable And Its Consumer-Owned Launch Environment.
#[derive(Clone, Debug)]
pub struct Managed {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
}

pub(super) struct Process {
    pub(super) child: Child,
    drain: JoinHandle<()>,
    pub(super) digest: [u8; 32],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Process {
    pub(super) async fn start(placement: &Managed) -> Result<(Self, String), Error> {
        let bytes = tokio::fs::read(&placement.executable).await?;
        let digest = Sha256::digest(bytes).into();
        let mut command = command(&placement.executable);
        command.args(&placement.arguments)
            .env("HARMOS_ADDRESS", "127.0.0.1:0")
            .envs(&placement.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("managed stdout is piped");
        let mut lines = BufReader::new(stdout).lines();
        let announced = loop {
            let line = lines.next_line().await?.ok_or_else(|| Error::Invalid("Plugin Exited Before Announcing Its Address".into()))?;
            let Some(address) = line.strip_prefix("harmos listening ") else { continue };
            let socket: SocketAddr = address.parse().map_err(|_| Error::Invalid("Invalid Announced Plugin Address".into()))?;
            if !socket.ip().is_loopback() || socket.port() == 0 {
                return Err(Error::Invalid("Managed Address Must Be Loopback with a Bound Port".into()));
            }
            break address.to_owned();
        };
        let override_address = placement.environment.get(&OsString::from("HARMOS_ADDRESS")).cloned();
        let address = match override_address {
            Some(address) if address != "127.0.0.1:0" => address.into_string().map_err(|_| Error::Invalid("HARMOS_ADDRESS Is Not UTF-8".into()))?,
            Some(_) => announced,
            None => announced,
        };
        let drain = tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
        Ok((Self { child, drain, digest }, address))
    }

    pub(super) async fn exited(&mut self) -> Result<ExitStatus, Error> {
        Ok(self.child.wait().await?)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.drain.abort();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
