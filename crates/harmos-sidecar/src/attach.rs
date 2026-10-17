/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[cfg(unix)]
pub(crate) mod control;
#[cfg(windows)]
#[path = "attach/windows.rs"]
pub(crate) mod control;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::ffi::OsString;
use std::future::Future;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};
// Local Imports
use crate::StreamDeclaration;
use crate::inventory::{Declaration, DeclarationError, Facility};

/* === Constants === */
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/* === Macros === */

/* === Type Aliases === */
type EndWork = Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>>;
type EndStart = Box<dyn FnOnce() -> EndWork + Send>;

/* === Structs === */
#[must_use = "retain the ending until physical settlement succeeds"]
pub struct Ending {
    receipt: Box<dyn End>,
}

struct Terminal {
    start: Option<EndStart>,
    work: Option<EndWork>,
    result: Option<Result<(), Arc<Error>>>,
}

struct ProcessEnd {
    child: Child,
    signaled: bool,
    completed: bool,
}

pub struct Attached {
    pub(super) control: Option<crate::transport::Io>,
    pub(super) write: Box<dyn AsyncWrite + Send + Unpin>,
    pub(super) read: Box<dyn AsyncRead + Send + Unpin>,
    pub(super) diagnostics: Option<Box<dyn AsyncRead + Send + Unpin>>,
    pub(super) end: Option<Ending>,
}

#[derive(Clone, Debug)]
pub struct Executable {
    program: PathBuf,
    declaration: Declaration,
    sha256: [u8; 32],
    arguments: Vec<OsString>,
    directory: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum ExecutableError {
    #[error("the sidecar executable `{path}` is not an absolute path")]
    Relative { path: PathBuf },
    #[error("the sidecar executable `{path}` cannot be read: {source}")]
    Unreadable { path: PathBuf, source: std::io::Error },
    #[error("the sidecar executable `{path}` is not a file")]
    NotFile { path: PathBuf },
    #[error("the sidecar executable `{path}` has no valid declaration: {source}")]
    Declaration {
        path: PathBuf,
        #[source]
        source: Box<DeclarationError>,
    },
}

/* === Traits === */
pub trait End: Send + 'static {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + '_>>;
}

pub trait Attach: Clone + Send + Sync + 'static {
    type Error: std::fmt::Display + Send + 'static;

    fn owner(&self) -> &str;

    fn publications(&self) -> Vec<StreamDeclaration> {
        Vec::new()
    }

    fn observers(&self) -> Vec<crate::inventory::observers::Declaration> {
        Vec::new()
    }

    fn operations(&self) -> harmos_domain::Result<Vec<crate::operations::Definition>> {
        Ok(Vec::new())
    }

    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<crate::operations::Contract>>> {
        Ok(None)
    }

    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<crate::inventory::contracts::Required>>> {
        Ok(None)
    }

    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[crate::operations::lifecycle::Descriptor; 2]>> {
        Ok(None)
    }

    fn observe_contract(&self) -> harmos_domain::Result<Option<crate::operations::Contract>> {
        Ok(None)
    }

    fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
        Ok(None)
    }

    fn attach(&self) -> impl Future<Output = Result<Attached, Self::Error>> + Send;
}

/* === Implementations === */
impl Ending {
    pub fn new(receipt: impl End) -> Self {
        Self { receipt: Box::new(receipt) }
    }

    pub fn terminal<F, U>(end: F) -> Self
    where
        F: FnOnce() -> U + Send + 'static,
        U: Future<Output = std::io::Result<()>> + Send + 'static,
    {
        Self::new(Terminal { start: Some(Box::new(move || Box::pin(end()))), work: None, result: None })
    }

    pub async fn settle(&mut self) -> std::io::Result<()> {
        self.receipt.settle().await
    }
}

impl End for Terminal {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            if let Some(result) = &self.result {
                return result.clone().map_err(Error::other);
            }
            if self.work.is_none() {
                let start = self.start.take().ok_or_else(|| Error::other("terminal ending construction did not return"))?;
                self.work = Some(start());
            }
            let result = self.work.as_mut().expect("original terminal work is retained").await;
            self.work = None;
            self.result = Some(result.map_err(Arc::new));
            self.result.as_ref().expect("terminal outcome is retained").clone().map_err(Error::other)
        })
    }
}

impl End for ProcessEnd {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            if self.completed {
                return Ok(());
            }
            if !self.signaled {
                self.child.start_kill()?;
                self.signaled = true;
            }
            self.child.wait().await?;
            self.completed = true;
            Ok(())
        })
    }
}

impl Attached {
    pub fn new(write: impl AsyncWrite + Send + Unpin + 'static, read: impl AsyncRead + Send + Unpin + 'static) -> Self {
        Self { control: None, write: Box::new(write), read: Box::new(read), diagnostics: None, end: None }
    }

    pub fn control(mut self, channel: crate::transport::Io) -> Self {
        self.control = Some(channel);
        self
    }

    pub fn diagnostics(mut self, read: impl AsyncRead + Send + Unpin + 'static) -> Self {
        self.diagnostics = Some(Box::new(read));
        self
    }

    pub fn ended_by<F, U>(self, end: F) -> Self
    where
        F: FnOnce() -> U + Send + 'static,
        U: Future<Output = ()> + Send + 'static,
    {
        self.ending(Ending::terminal(move || async move {
            end().await;
            Ok(())
        }))
    }

    pub fn ending(mut self, ending: Ending) -> Self {
        self.end = Some(ending);
        self
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        Box<dyn AsyncWrite + Send + Unpin>,
        Box<dyn AsyncRead + Send + Unpin>,
        Option<crate::transport::Io>,
        Option<Box<dyn AsyncRead + Send + Unpin>>,
        Option<Ending>,
    ) {
        (self.write, self.read, self.control, self.diagnostics, self.end)
    }
}

impl Executable {
    pub fn at(path: impl AsRef<Path>) -> Result<Self, ExecutableError> {
        let program = path.as_ref().to_path_buf();
        if !program.is_absolute() {
            return Err(ExecutableError::Relative { path: program });
        }
        let held = std::fs::metadata(&program).map_err(|source| ExecutableError::Unreadable { path: program.clone(), source })?;
        if !held.is_file() {
            return Err(ExecutableError::NotFile { path: program });
        }
        let bytes = std::fs::read(&program).map_err(|source| ExecutableError::Unreadable { path: program.clone(), source })?;
        let declaration = Declaration::inspect(&bytes, Facility::Sidecar)
            .map_err(|source| ExecutableError::Declaration { path: program.clone(), source: Box::new(source) })?;
        Ok(Self {
            program,
            declaration,
            sha256: Sha256::digest(&bytes).into(),
            arguments: Vec::new(),
            directory: None,
            environment: Vec::new(),
        })
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    pub fn path(&self) -> &Path {
        &self.program
    }

    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub fn working_directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    pub fn environment(&self) -> &[(OsString, OsString)] {
        &self.environment
    }

    pub fn argument(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.directory = Some(path.into());
        self
    }

    pub fn variable(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.push((name.into(), value.into()));
        self
    }
}

impl Attach for Executable {
    type Error = Error;

    fn owner(&self) -> &str {
        &self.declaration.id
    }

    fn publications(&self) -> Vec<StreamDeclaration> {
        self.declaration.streams.clone()
    }

    fn observers(&self) -> Vec<crate::inventory::observers::Declaration> {
        self.declaration.observers.clone()
    }

    fn operations(&self) -> harmos_domain::Result<Vec<crate::operations::Definition>> {
        Ok(self.declaration.operations.clone())
    }

    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<crate::operations::Contract>>> {
        Ok(Some(self.declaration.contracts.provides.clone()))
    }

    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<crate::inventory::contracts::Required>>> {
        Ok(Some(self.declaration.contracts.requires.clone()))
    }

    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[crate::operations::lifecycle::Descriptor; 2]>> {
        Ok(Some(self.declaration.contracts.lifecycle.clone()))
    }

    fn observe_contract(&self) -> harmos_domain::Result<Option<crate::operations::Contract>> {
        Ok(self.declaration.observe_contract())
    }

    fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
        Ok(Some(self.declaration.listeners.clone()))
    }

    async fn attach(&self) -> Result<Attached, Error> {
        let mut command = command(&self.program, &self.arguments, self.directory.as_deref(), &self.environment)?;

        #[cfg(any(unix, windows))]
        let control = control::prepare(&mut command)?;
        let mut child = command.spawn()?;
        drop(command);
        #[cfg(windows)]
        let control = control.io(child.id().expect("a spawned child has its process id"));
        let write = child.stdin.take().ok_or_else(|| Error::other("the sidecar was started without a standard input"))?;
        let read = child.stdout.take().ok_or_else(|| Error::other("the sidecar was started without a standard output"))?;
        let diagnostics = child.stderr.take();

        let ending = Ending::new(ProcessEnd { child, signaled: false, completed: false });
        let mut attached = Attached::new(write, read).ending(ending);
        #[cfg(any(unix, windows))]
        {
            attached = attached.control(control);
        }
        if let Some(diagnostics) = diagnostics {
            attached = attached.diagnostics(diagnostics);
        }
        Ok(attached)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn command(
    program: &Path, arguments: &[OsString], directory: Option<&Path>, environment: &[(OsString, OsString)],
) -> Result<Command, Error> {
    let mut command = Command::new(program);
    command.args(arguments).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
        command.env("SystemRoot", system_root()?);
        let temporary = std::env::temp_dir();
        command.env("TMP", &temporary).env("TEMP", &temporary);
    }
    if !cfg!(windows)
        && let Some(tmpdir) = std::env::var_os("TMPDIR").filter(|value| !value.is_empty())
    {
        command.env("TMPDIR", tmpdir);
    }
    command.envs(environment.iter().cloned());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }

    Ok(command)
}

#[cfg(windows)]
fn system_root() -> Result<OsString, Error> {
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;

    if let Some(root) = std::env::var_os("SystemRoot").filter(|value| !value.is_empty()) {
        return Ok(root);
    }

    let mut buffer = vec![0_u16; 260];
    loop {
        // SAFETY: the buffer is writable for the supplied number of UTF-16
        // units, and the API retains no pointer after this synchronous call.
        #[allow(unsafe_code)]
        let length = unsafe { GetSystemWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 {
            return Err(Error::last_os_error());
        }
        if (length as usize) < buffer.len() {
            return Ok(OsString::from_wide(&buffer[..length as usize]));
        }
        buffer.resize(length as usize, 0);
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_retains_explicit_configuration_without_spawning() {
        let arguments = [OsString::from("--plugin")];
        let directory = Path::new("work");
        let environment = [
            (OsString::from("HARMOS_EXPLICIT"), OsString::from("kept")),
            (OsString::from("TMPDIR"), OsString::from("chosen-unix-temp")),
            (OsString::from("TMP"), OsString::from("chosen-windows-temp")),
            (OsString::from("TEMP"), OsString::from("chosen-windows-temp")),
        ];
        let configured = command(Path::new("sidecar"), &arguments, Some(directory), &environment).unwrap();
        let configured = configured.as_std();
        assert_eq!(configured.get_program(), "sidecar");
        assert_eq!(configured.get_args().collect::<Vec<_>>(), [arguments[0].as_os_str()]);
        assert_eq!(configured.get_current_dir(), Some(directory));
        let variables = configured.get_envs().collect::<std::collections::BTreeMap<_, _>>();
        for (name, value) in &environment {
            assert_eq!(variables.get(name.as_os_str()), Some(&Some(value.as_os_str())));
        }
        assert!(!variables.contains_key(std::ffi::OsStr::new("PATH")));
        #[cfg(windows)]
        assert!(variables.contains_key(std::ffi::OsStr::new("SystemRoot")));
    }
}
