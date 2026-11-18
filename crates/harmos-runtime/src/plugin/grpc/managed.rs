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

mod spawner;
#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdout},
    task::JoinHandle,
    time::Instant,
};

// Local Imports
use super::Error;
use crate::{clock::Clock, runtime::StopParticipant};
use spawner::Spawner;
#[cfg(unix)]
use unix::command;
#[cfg(target_os = "windows")]
use windows::command;

/* === Constants === */

static SPAWNER: Spawner = Spawner::new();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Program, Arguments, Working Directory and Consumer-Owned Environment.
#[derive(Clone, Debug)]
pub struct Managed {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub directory: Option<PathBuf>,
    pub environment: BTreeMap<OsString, OsString>,
}

pub(super) struct Process {
    child: Option<Child>,
    lines: Option<Lines<BufReader<ChildStdout>>>,
    drain: Option<JoinHandle<()>>,
    executable: PathBuf,
    participant: Option<StopParticipant>,
    pub(super) digest: [u8; 32],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Managed {
    pub(crate) fn launch(launch: &[String], root: &Path) -> Result<Self, Error> {
        crate::bundle::validate_launch(launch).map_err(|error| Error::Invalid(format!("Start Managed Plugin: {error}")))?;
        let root = root.canonicalize().map_err(|error| Error::Invalid(format!("Resolve Bundle Root: {error}; reinstall the bundle")))?;
        let program = &launch[0];
        let executable = if let Some(relative) =
            crate::bundle::relative_program(program).map_err(|error| Error::Invalid(format!("Start Managed Plugin: {error}")))?
        {
            let executable = root.join(relative).canonicalize().map_err(|error| {
                Error::Invalid(format!("Start Managed Plugin: program {program}: {error}; install program {program} in the bundle"))
            })?;
            if !executable.starts_with(&root) {
                return Err(Error::Invalid(format!(
                    "Start Managed Plugin: program {program} escapes the bundle root; reinstall the bundle"
                )));
            }
            executable
        } else {
            path_program(program, &root, &env::var_os("PATH").unwrap_or_default())?
        };
        if !executable.is_file() {
            return Err(Error::Invalid(format!("Start Managed Plugin: program {program} is not a file; install {program}")));
        }
        let batch =
            executable.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("bat") || extension.eq_ignore_ascii_case("cmd"));
        if cfg!(target_os = "windows") && batch {
            return Err(Error::Invalid(format!(
                "Start Managed Plugin: program {program} requires a shell; install and invoke a native interpreter directly"
            )));
        }
        Ok(Self {
            executable,
            arguments: launch[1..].iter().map(OsString::from).collect(),
            directory: Some(root),
            environment: [("HARMOS_ADDRESS".into(), "127.0.0.1:0".into())].into(),
        })
    }
}

impl Process {
    pub(super) async fn spawn(placement: &Managed, participant: StopParticipant) -> Result<Self, Error> {
        let bytes = tokio::fs::read(&placement.executable)
            .await
            .map_err(|error| Error::Invalid(format!("Start Managed Plugin: read {}: {error}", placement.executable.display())))?;
        let digest = Sha256::digest(bytes).into();
        let mut command = command(&placement.executable);
        if let Some(directory) = &placement.directory {
            command.current_dir(directory);
        }
        command
            .args(&placement.arguments)
            .envs(&placement.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let executable = placement.executable.clone();
        SPAWNER
            .spawn(command, move |mut child| {
                let stdout = child.stdout.take().expect("managed stdout is piped");
                Self {
                    child: Some(child),
                    lines: Some(BufReader::new(stdout).lines()),
                    drain: None,
                    executable,
                    participant: Some(participant),
                    digest,
                }
            })
            .await
            .map_err(|error| Error::Invalid(format!("Start Managed Plugin: spawn {}: {error}", placement.executable.display())))
    }

    pub(super) async fn ready(&mut self, placement: &Managed) -> Result<String, Error> {
        let process = format!("{} (pid {:?})", self.executable.display(), self.id());
        let lines = self.lines.as_mut().expect("managed stdout is owned until readiness");
        let announced = loop {
            tracing::debug!(%process, "waiting for managed plugin readiness line");
            let line = lines.next_line().await;
            tracing::debug!(
                %process,
                outcome = match &line {
                    Ok(Some(_)) => "line",
                    Ok(None) => "eof",
                    Err(_) => "failed",
                },
                error = ?line.as_ref().err(),
                "managed plugin readiness line wait completed"
            );
            let line =
                line.map_err(|error| Error::Invalid(format!("Read Managed Plugin Readiness: process {process}: {error}")))?.ok_or_else(
                    || Error::Invalid(format!("Read Managed Plugin Readiness: process {process} exited before announcing its address")),
                )?;
            let Some(address) = line.strip_prefix("harmos listening ") else { continue };
            let socket: SocketAddr = address
                .parse()
                .map_err(|_| Error::Invalid(format!("Read Managed Plugin Readiness: process {process} announced an invalid address")))?;
            if !socket.ip().is_loopback() || socket.port() == 0 {
                return Err(Error::Invalid(format!(
                    "Read Managed Plugin Readiness: process {process} must announce loopback with a bound port"
                )));
            }
            break address.to_owned();
        };
        let override_address =
            placement.environment.get(&OsString::from("HARMOS_ADDRESS")).cloned().or_else(|| std::env::var_os("HARMOS_ADDRESS"));
        let address = match override_address {
            Some(address) if address != "127.0.0.1:0" => address
                .into_string()
                .map_err(|_| Error::Invalid(format!("Read Managed Plugin Readiness: process {process}: HARMOS_ADDRESS is not UTF-8")))?,
            Some(_) => announced,
            None => announced,
        };
        let mut lines = self.lines.take().expect("managed stdout is owned until readiness");
        self.drain = Some(tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} }));
        Ok(address)
    }

    pub(super) fn id(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    /// Kill the Child, Then Reap It Within a Fresh `grace` Measured From the Kill.
    pub(super) async fn terminate(&mut self, clock: &Clock, grace: Duration, action: &str) -> Result<(), Error> {
        let process = format!("{} (pid {:?})", self.executable.display(), self.id());
        let exit = self
            .child
            .as_mut()
            .expect("managed child is owned until reaped")
            .try_wait()
            .map_err(|error| self.reap_failure(format!("{action}: inspect process {process}: {error}; retain its installation")))?;
        if exit.is_some() {
            return Ok(());
        }
        self.child
            .as_mut()
            .expect("managed child is owned until reaped")
            .start_kill()
            .map_err(|error| self.reap_failure(format!("{action}: terminate process {process}: {error}; retain its installation")))?;
        clock.timeout_at(clock.now() + grace, self.exited()).await.map_err(|_| {
            self.reap_failure(format!("{action}: process {process} outlived stop_grace after it was killed; retain its installation"))
        })??;
        Ok(())
    }

    /// Await the Exit a Stop Request Asked For Until `deadline`, Then Terminate the Child.
    pub(super) async fn stop(
        &mut self, clock: &Clock, deadline: Instant, grace: Duration, action: &'static str,
    ) -> Result<ExitStatus, Error> {
        match clock.timeout_at(deadline, self.exited()).await {
            Ok(exit) => exit,
            Err(_) => {
                self.terminate(clock, grace, action).await?;
                Err(Error::Deadline { action })
            }
        }
    }

    fn reap_failure(&self, message: String) -> Error {
        if let Some(participant) = &self.participant {
            participant.fail(Arc::new(crate::Error::TaskFailed {
                domain: "managed plugin process",
                source: Arc::new(Error::Reap(message.clone())),
            }));
        }
        Error::Reap(message)
    }

    pub(super) async fn exited(&mut self) -> Result<ExitStatus, Error> {
        let process = format!("{} (pid {:?})", self.executable.display(), self.id());
        self.child
            .as_mut()
            .expect("managed child is owned until reaped")
            .wait()
            .await
            .map_err(|error| self.reap_failure(format!("Reap Managed Plugin: process {process}: {error}; retain its installation")))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Some(drain) = self.drain.take() {
            drain.abort();
        }
        if let Some(mut child) = self.child.take() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            let participant = self.participant.take();
            let process = format!("{} (pid {:?})", self.executable.display(), child.id());
            // Cancellation Cannot Discard the Native Exit Obligation.
            let _ = child.start_kill();
            tokio::spawn(async move {
                if let Err(error) = child.wait().await {
                    tracing::error!(%process, %error, "Reap Managed Plugin Process");
                    if let Some(participant) = &participant {
                        participant.fail(Arc::new(crate::Error::TaskFailed {
                            domain: "managed plugin process",
                            source: Arc::new(Error::Reap(format!(
                                "Reap Managed Plugin: process {process}: {error}; retain its installation"
                            ))),
                        }));
                    }
                }
                drop(participant);
            });
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn path_program(program: &str, root: &Path, path: &OsStr) -> Result<PathBuf, Error> {
    for directory in env::split_paths(path) {
        let mut candidate = root.join(directory).join(program);
        if cfg!(target_os = "windows") && candidate.extension().is_none() {
            candidate.set_extension("exe");
        }
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(Error::Invalid(format!("Start Managed Plugin: program {program} is missing; install {program} and make it available on PATH")))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::path_program;

    #[test]
    fn path_resolution_and_missing_program() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("programs");
        fs::create_dir(&directory).unwrap();
        let program = directory.join(format!("interpreter{}", env::consts::EXE_SUFFIX));
        fs::write(&program, b"program").unwrap();
        let path = env::join_paths([root.path().join("absent"), directory]).unwrap();
        assert_eq!(path_program("interpreter", root.path(), &path).unwrap(), program);
        let error = path_program("missing-interpreter", root.path(), &path).unwrap_err();
        assert!(error.to_string().contains("install missing-interpreter"));
    }
}
