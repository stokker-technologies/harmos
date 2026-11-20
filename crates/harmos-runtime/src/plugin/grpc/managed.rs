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
    collections::{BTreeMap, VecDeque},
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Child,
    sync::watch,
    task::JoinHandle,
    time::Instant,
};

// Local Imports
use super::{Error, Launch, Launches, StartupOutput};
use crate::{clock::Clock, runtime::StopParticipant};
use spawner::Spawner;
#[cfg(unix)]
use unix::command;
#[cfg(target_os = "windows")]
use windows::command;

/* === Constants === */

static SPAWNER: Spawner = Spawner::new();

/// One Read From a Child's Output Pipe.
const READ_BYTES: usize = 4096;

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
    id: Option<u32>,
    output: Output,
    executable: PathBuf,
    participant: Option<StopParticipant>,
    pub(super) digest: [u8; 32],
}

/// Drains Both Output Pipes for the Child's Whole Life So Neither Can Block It.
struct Output {
    drains: Vec<JoinHandle<()>>,
    tail: Arc<Mutex<Option<Tail>>>,
    errors: watch::Receiver<bool>,
}

/// The Bounded Standard-Error Tail Kept Until Activation Settles.
struct Tail {
    lines: VecDeque<String>,
    bytes: usize,
    dropped: bool,
    bound: StartupOutput,
}

/* === Enums === */

#[derive(Clone, Copy)]
enum Stream {
    Output,
    Errors,
}

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
            environment: BTreeMap::new(),
        })
    }
}

impl Process {
    /// Reserve a Launch Credential for `owner`, Then Start the Child With It.
    ///
    /// The returned [`Launch`] belongs inside the load's work, so the load's end revokes it; the
    /// process belongs to the owner that settles it afterwards.
    pub(super) async fn spawn(
        placement: &Managed, participant: StopParticipant, launches: &Launches, output: StartupOutput, owner: &str,
    ) -> Result<(Self, Launch), Error> {
        let bytes = tokio::fs::read(&placement.executable)
            .await
            .map_err(|error| Error::Invalid(format!("Start Managed Plugin: read {}: {error}", placement.executable.display())))?;
        let digest = Sha256::digest(bytes).into();
        let launch = launches.reserve(owner).await?;
        let mut command = command(&placement.executable);
        if let Some(directory) = &placement.directory {
            command.current_dir(directory);
        }
        // The Launch Credential Follows the Placement's Environment, Which Cannot Replace It.
        command
            .args(&placement.arguments)
            .envs(&placement.environment)
            .envs(launch.environment())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let executable = placement.executable.clone();
        let process = SPAWNER
            .spawn(command, move |mut child| {
                let id = child.id();
                let output = Output::drain(&mut child, &description(&executable, id), output);
                Self {
                    child: Some(child),
                    id,
                    output,
                    executable,
                    participant: Some(participant),
                    digest,
                }
            })
            .await
            .map_err(|error| Error::Invalid(format!("Start Managed Plugin: spawn {}: {error}", placement.executable.display())))?;
        Ok((process, launch))
    }

    /// Await the Announcement; the Child's Own Exit Wins Over Waiting for It.
    pub(super) async fn ready(&mut self, launch: &mut Launch) -> Result<String, Error> {
        let process = self.description();
        let child = self.child.as_mut().expect("managed child is owned until reaped");
        tracing::debug!(%process, "waiting for managed plugin announcement");
        let announced = tokio::select! {
            biased;
            exit = child.wait() => Err(exit),
            announced = launch.announced() => Ok(announced),
        };
        match announced {
            Ok(Ok(address)) => Ok(address.to_string()),
            Ok(Err(error)) => Err(Error::Invalid(format!("{error} (process {process})"))),
            Err(exit) => {
                let status = exit.map_err(|error| {
                    self.reap_failure(format!("Reap Managed Plugin: process {process}: {error}; retain its installation"))
                })?;
                // The Child Has Exited, So Its Standard Error Ends Unless a Descendant Still Holds It.
                self.output.closed().await;
                let diagnostics = self.diagnostics(Some(status));
                Err(Error::Exited { process, status, diagnostics })
            }
        }
    }

    /// Activation Settled: Stop Retaining the Standard-Error Tail; Both Pipes Keep Draining.
    pub(super) fn settled(&self) {
        self.output.tail.lock().expect("managed output lock is healthy").take();
    }

    /// Terminate a Child Whose Startup Failed; a Missed Deadline Gains Its Exit Status and Output Tail.
    pub(super) async fn fail(&mut self, clock: &Clock, grace: Duration, action: &str, error: Error) -> Result<Error, Error> {
        self.terminate(clock, grace, action).await?;
        let Error::LoadDeadline { plugin, timeout, configured_by, .. } = error else {
            self.settled();
            return Ok(error);
        };
        let _ = clock.timeout(grace, self.output.closed()).await;
        let status = self.child.as_mut().and_then(|child| child.try_wait().ok().flatten());
        let diagnostics = self.diagnostics(status);
        Ok(Error::LoadDeadline {
            plugin,
            timeout,
            configured_by,
            diagnostics,
        })
    }

    pub(super) fn id(&self) -> Option<u32> {
        self.id
    }

    /// Kill the Child, Then Reap It Within a Fresh `grace` Measured From the Kill.
    pub(super) async fn terminate(&mut self, clock: &Clock, grace: Duration, action: &str) -> Result<(), Error> {
        let process = self.description();
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

    fn description(&self) -> String {
        description(&self.executable, self.id)
    }

    /// Describe the Process and Its Exit, Then Release the Retained Standard-Error Tail Into the Text.
    fn diagnostics(&self, status: Option<ExitStatus>) -> String {
        let status = status.map_or_else(|| "unknown".to_owned(), |status| status.to_string());
        let mut diagnostics = format!("process {}; exit status {status}", self.description());
        if let Some(tail) = self.output.tail.lock().expect("managed output lock is healthy").take() {
            let omitted = if tail.dropped { ", earlier output omitted" } else { "" };
            diagnostics.push_str(&format!("; standard error tail ({} lines{omitted}):", tail.lines.len()));
            for line in tail.lines {
                diagnostics.push('\n');
                diagnostics.push_str(&line);
            }
        }
        diagnostics
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
        let process = self.description();
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
        for drain in self.output.drains.drain(..) {
            drain.abort();
        }
        if let Some(mut child) = self.child.take() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            let participant = self.participant.take();
            let process = description(&self.executable, self.id);
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

impl Output {
    /// Take Both Pipes of `child` and Drain Them Until End of File.
    fn drain(child: &mut Child, process: &str, bound: StartupOutput) -> Self {
        let tail = Arc::new(Mutex::new(Some(Tail {
            lines: VecDeque::new(),
            bytes: 0,
            dropped: false,
            bound,
        })));
        let (closed, errors) = watch::channel(false);
        let mut drains = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            drains.push(tokio::spawn(forward(stdout, Stream::Output, process.to_owned(), None, bound.bytes.get())));
        }
        match child.stderr.take() {
            Some(stderr) => {
                let tail = tail.clone();
                let process = process.to_owned();
                drains.push(tokio::spawn(async move {
                    forward(stderr, Stream::Errors, process, Some(tail), bound.bytes.get()).await;
                    closed.send_replace(true);
                }));
            }
            None => {
                closed.send_replace(true);
            }
        }
        Self { drains, tail, errors }
    }

    async fn closed(&mut self) {
        let _ = self.errors.wait_for(|closed| *closed).await;
    }
}

impl Tail {
    fn push(&mut self, line: String) {
        self.bytes += line.len();
        self.lines.push_back(line);
        while self.lines.len() > self.bound.lines.get() || self.bytes > self.bound.bytes.get() {
            let removed = self.lines.pop_front().expect("an over-bound tail holds a line");
            self.bytes -= removed.len();
            self.dropped = true;
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// The One Description of a Managed Process: Its Executable and, When Known, Its Process Identifier.
fn description(executable: &Path, id: Option<u32>) -> String {
    match id {
        Some(id) => format!("{} (pid {id})", executable.display()),
        None => executable.display().to_string(),
    }
}

/// Forward Each Line of One Pipe to the Host Log at Debug; Standard Error Also Fills the Tail While Retained.
///
/// A line longer than `line_bytes` is cut there, so a child writing without newlines cannot grow host memory.
async fn forward(
    mut pipe: impl AsyncRead + Unpin, stream: Stream, process: String, tail: Option<Arc<Mutex<Option<Tail>>>>, line_bytes: usize,
) {
    let emit = |line: &[u8]| {
        let text = String::from_utf8_lossy(line.strip_suffix(b"\r").unwrap_or(line)).into_owned();
        match stream {
            Stream::Output => tracing::debug!(target: "harmos::plugin::stdout", %process, "{text}"),
            Stream::Errors => tracing::debug!(target: "harmos::plugin::stderr", %process, "{text}"),
        }
        if let Some(tail) = &tail
            && let Some(tail) = tail.lock().expect("managed output lock is healthy").as_mut()
        {
            tail.push(text);
        }
    };
    let mut buffer = vec![0; READ_BYTES];
    let mut line = Vec::new();
    while let Ok(read) = pipe.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' {
                emit(&line);
                line.clear();
            } else if line.len() < line_bytes {
                line.push(byte);
            }
        }
    }
    if !line.is_empty() {
        emit(&line);
    }
}

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
    use std::{env, fs, num::NonZeroUsize};

    use super::{StartupOutput, Tail, description, path_program};

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

    #[test]
    fn the_tail_keeps_the_newest_lines_within_both_bounds() {
        // Bound the Tail to Three Lines and Eleven Bytes.
        let mut tail = Tail {
            lines: Default::default(),
            bytes: 0,
            dropped: false,
            bound: StartupOutput {
                lines: NonZeroUsize::new(3).unwrap(),
                bytes: NonZeroUsize::new(11).unwrap(),
            },
        };
        for line in ["one", "two", "three"] {
            tail.push(line.into());
        }
        assert!(!tail.dropped);

        // The Line Bound and Then the Byte Bound Push Out the Oldest Lines.
        tail.push("four".into());
        assert_eq!(tail.lines, ["three", "four"]);
        tail.push("abcdefghij".into());
        assert_eq!(tail.lines, ["abcdefghij"]);
        assert!(tail.dropped);
    }

    #[test]
    fn a_process_description_prints_its_identifier_as_a_number_or_omits_it() {
        let executable = std::path::Path::new("plugin");
        let known = description(executable, Some(32212));
        assert!(known.contains("32212") && !known.contains("Some"));
        assert!(!description(executable, None).contains("pid"));
    }
}
