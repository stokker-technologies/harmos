/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Cargo And Its Process Tree Until Successful Or Cancelled Settlement.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::Read;
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// Third Party Imports
use process_wrap::std::{ChildWrapper, CommandWrap};

// Local Imports
use super::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct BuildCancellation {
    stopped: Arc<AtomicBool>,
}

/// Bounds for an authoring subprocess whose output must be retained.
#[derive(Clone, Copy, Debug)]
pub struct ProcessSettings {
    pub timeout: Option<Duration>,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

pub(super) struct Process {
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    finished: Arc<AtomicBool>,
    monitor: Option<JoinHandle<std::io::Result<()>>>,
    failed: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl BuildCancellation {
    pub fn cancel(&self) {
        self.stopped.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    pub(super) fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
}

impl Process {
    pub(super) fn spawn(command: Command, cancellation: &BuildCancellation) -> Result<Self, Error> {
        Self::spawn_until(command, cancellation, None)
    }

    fn spawn_until(command: Command, cancellation: &BuildCancellation, timeout: Option<Duration>) -> Result<Self, Error> {
        cancellation.check()?;
        let mut command = CommandWrap::from(command);
        #[cfg(unix)]
        command.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        {
            // JobObject Replaces Native Flags; Its Wrapper Preserves Explicit Policy.
            let mut flags = process_wrap::std::CreationFlags(Default::default());
            flags.0.0 = 0x08000000;
            command.wrap(flags).wrap(process_wrap::std::JobObject);
        }
        let child = Arc::new(Mutex::new(command.spawn()?));
        let retained = child.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let completed = finished.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let fault = failed.clone();
        let timed_out = Arc::new(AtomicBool::new(false));
        let expired = timed_out.clone();
        let started = Instant::now();
        let cancellation = cancellation.clone();
        let monitor = std::thread::spawn(move || {
            while !completed.load(Ordering::Acquire) {
                let elapsed = timeout.is_some_and(|timeout| started.elapsed() >= timeout);
                if elapsed {
                    expired.store(true, Ordering::Release);
                }
                if cancellation.is_cancelled() || fault.load(Ordering::Acquire) || elapsed {
                    return retained.lock().expect("process owner is healthy").start_kill();
                }
                std::thread::park_timeout(Duration::from_millis(20));
            }
            Ok(())
        });
        Ok(Self {
            child,
            finished,
            monitor: Some(monitor),
            failed,
            timed_out,
        })
    }

    pub(super) fn stdout(&self) -> std::process::ChildStdout {
        self.child.lock().expect("process owner is healthy").stdout().take().expect("Cargo stdout was piped")
    }

    pub(super) fn kill(&self) -> Result<(), Error> {
        let mut child = self.child.lock().expect("process owner is healthy");
        if child.try_wait()?.is_none() {
            child.start_kill()?;
        }
        Ok(())
    }

    pub(super) fn wait(&mut self) -> Result<ExitStatus, Error> {
        let status = self.wait_status()?;
        self.finish_monitor()?;
        Ok(status)
    }

    fn wait_status(&mut self) -> Result<ExitStatus, Error> {
        loop {
            let status = self.child.lock().expect("process owner is healthy").try_wait()?;
            if let Some(status) = status {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn finish_monitor(&mut self) -> Result<(), Error> {
        self.finished.store(true, Ordering::Release);
        if let Some(monitor) = self.monitor.take() {
            monitor.thread().unpark();
            monitor.join().map_err(|_| std::io::Error::other("process cancellation monitor panicked"))??;
        }
        Ok(())
    }

    pub(super) fn output(self) -> Result<Output, Error> {
        self.output_bounded(usize::MAX, usize::MAX)
    }

    fn output_bounded(mut self, stdout_limit: usize, stderr_limit: usize) -> Result<Output, Error> {
        let mut stdout = self.stdout();
        let mut stderr = self.child.lock().expect("process owner is healthy").stderr().take().expect("Cargo stderr was piped");
        let fault = self.failed.clone();
        let stdout = std::thread::spawn(move || read(&mut stdout, stdout_limit, &fault));
        let fault = self.failed.clone();
        let stderr = std::thread::spawn(move || read(&mut stderr, stderr_limit, &fault));
        let status = self.wait_status();
        let cleanup = if status.is_err() { self.kill().and_then(|_| self.wait().map(|_| ())) } else { Ok(()) };
        let stdout = stdout.join().map_err(|_| std::io::Error::other("Cargo stdout reader panicked"));
        let stderr = stderr.join().map_err(|_| std::io::Error::other("Cargo stderr reader panicked"));
        self.finish_monitor()?;
        cleanup?;
        if self.timed_out.load(Ordering::Acquire) {
            return Err(Error::Timeout);
        }
        Ok(Output {
            status: status?,
            stdout: stdout??,
            stderr: stderr??,
        })
    }
}

pub fn capture(mut command: Command, settings: ProcessSettings, cancellation: &BuildCancellation) -> Result<Output, Error> {
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let result =
        Process::spawn_until(command, cancellation, settings.timeout)?.output_bounded(settings.stdout_bytes, settings.stderr_bytes);
    cancellation.check()?;
    let output = result?;
    if output.status.success() { Ok(output) } else { Err(Error::Cargo(String::from_utf8_lossy(&output.stderr).into_owned())) }
}

fn read(reader: &mut impl Read, limit: usize, failed: &AtomicBool) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let result = reader.take(limit.saturating_add(1) as u64).read_to_end(&mut bytes);
    if result.is_err() || bytes.len() > limit {
        failed.store(true, Ordering::Release);
    }
    result?;
    if bytes.len() > limit {
        return Err(std::io::Error::other("subprocess output exceeds its byte limit"));
    }
    Ok(bytes)
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Err(error) = self.kill().and_then(|_| self.wait().map(|_| ())) {
            tracing::error!(%error, "retained Cargo process cleanup failed");
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
