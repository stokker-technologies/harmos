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
use std::time::Duration;

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

pub(super) struct Process {
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    finished: Arc<AtomicBool>,
    monitor: Option<JoinHandle<std::io::Result<()>>>,
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
        let cancellation = cancellation.clone();
        let monitor = std::thread::spawn(move || {
            while !completed.load(Ordering::Acquire) {
                if cancellation.is_cancelled() {
                    return retained.lock().expect("process owner is healthy").start_kill();
                }
                std::thread::park_timeout(Duration::from_millis(20));
            }
            Ok(())
        });
        Ok(Self { child, finished, monitor: Some(monitor) })
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
        loop {
            let status = self.child.lock().expect("process owner is healthy").try_wait()?;
            if let Some(status) = status {
                self.finished.store(true, Ordering::Release);
                if let Some(monitor) = self.monitor.take() {
                    monitor.thread().unpark();
                    monitor.join().map_err(|_| std::io::Error::other("process cancellation monitor panicked"))??;
                }
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub(super) fn output(mut self) -> Result<Output, Error> {
        let mut stdout = self.stdout();
        let mut stderr = self.child.lock().expect("process owner is healthy").stderr().take().expect("Cargo stderr was piped");
        let stdout = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let stderr = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let status = self.wait();
        let cleanup = if status.is_err() { self.kill().and_then(|_| self.wait().map(|_| ())) } else { Ok(()) };
        let stdout = stdout.join().map_err(|_| std::io::Error::other("Cargo stdout reader panicked"));
        let stderr = stderr.join().map_err(|_| std::io::Error::other("Cargo stderr reader panicked"));
        cleanup?;
        Ok(Output { status: status?, stdout: stdout??, stderr: stderr?? })
    }
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
