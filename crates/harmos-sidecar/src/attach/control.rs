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
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;

// Third Party Imports
use tokio::process::Command;

// Local Imports
use crate::transport::Io;

/* === Constants === */
const DESCRIPTOR: i32 = 198;
const VARIABLE: &str = "HARMOS_CONTROL_V2";
const VALUE: &str = "unix-fd:198";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
#[allow(unsafe_code)]
pub(super) fn prepare(command: &mut Command) -> io::Result<Io> {
    let (parent, child) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    child.set_nonblocking(true)?;
    // Duplicate Above The Destination So dup2 Cannot Preserve CLOEXEC Or Clobber Its Source.
    let source = duplicate(child.as_raw_fd(), DESCRIPTOR + 1)?;
    command.env(VARIABLE, VALUE);
    // SAFETY: The Closure Owns The Source Through spawn; dup2 Is Async-Signal-Safe.
    // Only The Child Descriptor Table Is Changed. The Source Closes On Successful exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(source.as_raw_fd(), DESCRIPTOR) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    io(parent)
}

#[cfg(feature = "kit")]
#[allow(unsafe_code)]
pub(crate) fn inherited() -> io::Result<Option<Io>> {
    let Some(value) = std::env::var_os(VARIABLE) else {
        return Ok(None);
    };
    if value != VALUE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported inherited control channel",
        ));
    }
    let descriptor = duplicate(DESCRIPTOR, 0)?;
    // SAFETY: The Successful Duplication Validated The Reserved Descriptor. The Executable
    // Runner Owns This Protocol Slot Exclusively; Close It So EOF Tracks The Returned Owner.
    if unsafe { libc::close(DESCRIPTOR) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = UnixStream::from(descriptor);
    stream.set_nonblocking(true)?;
    io(stream).map(Some)
}

/* === Private Functions === */
#[allow(unsafe_code)]
fn duplicate(descriptor: i32, minimum: i32) -> io::Result<OwnedFd> {
    // SAFETY: fcntl Validates The Input; On Success It Returns A New Exclusively Owned FD.
    let duplicated = unsafe { libc::fcntl(descriptor, libc::F_DUPFD_CLOEXEC, minimum) };
    if duplicated < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Ownership Of The Successful fcntl Result Has Not Been Transferred Elsewhere.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
}

fn io(stream: UnixStream) -> io::Result<Io> {
    let (read, write) = tokio::net::UnixStream::from_std(stream)?.into_split();
    Ok((Box::new(read), Box::new(write)))
}

/* === Tests === */
