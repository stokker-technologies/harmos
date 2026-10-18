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
use std::future::Future;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

// Third Party Imports
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::process::Command;
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;

// Local Imports
use crate::transport::Io;

/* === Constants === */
const VARIABLE: &str = "HARMOS_CONTROL_V2";

/* === Macros === */

/* === Type Aliases === */
type Connection = Pin<Box<dyn Future<Output = io::Result<NamedPipeServer>> + Send>>;

/* === Structs === */
pub(super) struct Prepared(NamedPipeServer);

struct Pipe {
    state: State,
}

/* === Enums === */
enum State {
    Connecting(Connection),
    Connected(NamedPipeServer),
    Failed(io::ErrorKind, String),
}

/* === Traits === */

/* === Implementations === */
impl Prepared {
    pub(super) fn io(self, child: u32) -> Io {
        let connection = Box::pin(async move {
            self.0.connect().await?;
            verify_child(&self.0, child)?;
            Ok(self.0)
        });
        let (read, write) = tokio::io::split(Pipe { state: State::Connecting(connection) });
        (Box::new(read), Box::new(write))
    }
}

impl Pipe {
    fn connected(&mut self, context: &mut Context<'_>) -> Poll<io::Result<&mut NamedPipeServer>> {
        if let State::Connecting(connection) = &mut self.state {
            self.state = match ready!(connection.as_mut().poll(context)) {
                Ok(pipe) => State::Connected(pipe),
                Err(error) => State::Failed(error.kind(), error.to_string()),
            };
        }
        match &mut self.state {
            State::Connected(pipe) => Poll::Ready(Ok(pipe)),
            State::Failed(kind, message) => Poll::Ready(Err(io::Error::new(*kind, message.clone()))),
            State::Connecting(_) => unreachable!("connection is retained while pending"),
        }
    }
}

impl AsyncRead for Pipe {
    fn poll_read(self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let pipe = ready!(self.get_mut().connected(context))?;
        Pin::new(pipe).poll_read(context, buffer)
    }
}

impl AsyncWrite for Pipe {
    fn poll_write(self: Pin<&mut Self>, context: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        let pipe = ready!(self.get_mut().connected(context))?;
        Pin::new(pipe).poll_write(context, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let pipe = ready!(self.get_mut().connected(context))?;
        Pin::new(pipe).poll_flush(context)
    }
    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let pipe = ready!(self.get_mut().connected(context))?;
        Pin::new(pipe).poll_shutdown(context)
    }
}

/* === Public Functions === */
pub(super) fn prepare(command: &mut Command) -> io::Result<Prepared> {
    let mut entropy = [0; 32];
    getrandom::fill(&mut entropy).map_err(io::Error::other)?;
    let nonce: String = entropy.iter().map(|byte| format!("{byte:02x}")).collect();
    let identity = format!("{}:{nonce}", std::process::id());
    let server = ServerOptions::new().first_pipe_instance(true).max_instances(1).reject_remote_clients(true).create(name(&identity))?;
    command.env(VARIABLE, format!("windows-pipe:{identity}"));
    Ok(Prepared(server))
}

#[cfg(feature = "kit")]
#[allow(unsafe_code)]
pub(crate) fn inherited() -> io::Result<Option<Io>> {
    use tokio::net::windows::named_pipe::ClientOptions;
    use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
    let Some(value) = std::env::var_os(VARIABLE) else {
        return Ok(None);
    };
    let identity = value
        .to_str()
        .and_then(|value| value.strip_prefix("windows-pipe:"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unsupported Windows control channel"))?;
    let (parent, nonce) =
        identity.split_once(':').ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid control channel identity"))?;
    let parent = parent
        .parse::<u32>()
        .ok()
        .filter(|id| *id != 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid control parent process"))?;
    if nonce.len() != 64 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid control channel nonce"));
    }
    let pipe = ClientOptions::new().open(name(identity))?;
    let mut actual = 0;
    // SAFETY: The Live Client Owns The Pipe Handle And actual Is Writable For This Call.
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut actual) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if actual != parent {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "control pipe belongs to another parent"));
    }
    let (read, write) = tokio::io::split(pipe);
    Ok(Some((Box::new(read), Box::new(write))))
}

/* === Private Functions === */
fn name(identity: &str) -> String {
    format!(r"\\.\pipe\harmos-control-{}", identity.replace(':', "-"))
}

#[allow(unsafe_code)]
fn verify_child(pipe: &NamedPipeServer, expected: u32) -> io::Result<()> {
    let mut actual = 0;
    // SAFETY: The Live Server Owns The Connected Handle And actual Is Writable For This Call.
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut actual) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if actual != expected {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "control pipe connected to another child"));
    }
    Ok(())
}

/* === Tests === */
