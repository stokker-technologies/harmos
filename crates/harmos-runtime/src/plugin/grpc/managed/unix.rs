/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Process Startup

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;
#[cfg(target_os = "linux")]
use std::sync::{OnceLock, mpsc};

// Third Party Imports
use tokio::process::{Child, Command};
#[cfg(target_os = "linux")]
use tokio::{runtime::Handle, sync::oneshot};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

#[cfg(target_os = "linux")]
type Spawn = (Handle, Command, oneshot::Sender<std::io::Result<Child>>);

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn command(executable: &Path) -> Command {
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut command = Command::new(executable);
    #[cfg(target_os = "linux")]
    die_with_parent(&mut command);
    command
}

/// Spawn Where the Child's Parent-Death Signal Follows the Host Process, Not a Worker Thread.
#[cfg(target_os = "linux")]
pub(super) async fn spawn(command: Command) -> std::io::Result<Child> {
    // Linux Delivers PR_SET_PDEATHSIG When the Forking Thread Exits, and Runtime Workers
    // May Retire; One Spawner Thread Lives for the Whole Process Instead.
    static SPAWNER: OnceLock<mpsc::Sender<Spawn>> = OnceLock::new();
    let spawner = SPAWNER.get_or_init(|| {
        let (sender, requests) = mpsc::channel::<Spawn>();
        std::thread::Builder::new()
            .name("harmos-managed-spawner".into())
            .spawn(move || {
                for (runtime, mut command, reply) in requests {
                    let _entered = runtime.enter();
                    let _ = reply.send(command.spawn());
                }
            })
            .expect("managed plugin spawner thread starts");
        sender
    });
    let (reply, spawned) = oneshot::channel();
    spawner
        .send((Handle::current(), command, reply))
        .map_err(|_| std::io::Error::other("Start Managed Plugin: spawner thread has stopped"))?;
    spawned.await.map_err(|_| std::io::Error::other("Start Managed Plugin: spawner thread dropped the request"))?
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(super) async fn spawn(mut command: Command) -> std::io::Result<Child> {
    command.spawn()
}

/// Kill the Child When the Host Process Dies, Including the Race Where It Died Before `prctl`.
#[cfg(target_os = "linux")]
fn die_with_parent(command: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: The Hook Calls Only Async-Signal-Safe Functions and Allocates Nothing.
    #[allow(unsafe_code)]
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

/* === Tests === */
