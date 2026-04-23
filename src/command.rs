//! Running external commands.

use std::{
    ffi::OsStr,
    fmt,
    process::{Command, Output},
    str::Utf8Error,
};

use color_eyre::{Section, SectionExt};

use crate::display;

#[allow(dead_code)] // TODO: remove
pub(crate) fn output(
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<String, CmdError> {
    let (command, mut output) = get_output(make_command(program, args))?;
    match String::from_utf8(output.stdout) {
        Ok(string) => Ok(string),
        Err(error) => {
            let utf8_error = error.utf8_error();
            output.stdout = error.into_bytes();
            Err(CmdError::utf8(command, output, utf8_error))
        }
    }
}

pub(crate) fn output_json<T: serde::de::DeserializeOwned>(
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<T, CmdError> {
    let (command, output) = get_output(make_command(program, args))?;
    match serde_json::from_slice(&output.stdout) {
        Ok(value) => Ok(value),
        Err(error) => Err(CmdError::json(command, output, error)),
    }
}

fn make_command(
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd
}

fn get_output(mut command: Command) -> Result<(Command, Output), CmdError> {
    tracing::trace!(
        program = %command.get_program().display(),
        args = %display_args(&command),
        "executing external program",
    );

    let output = match command.output() {
        Ok(output) => output,
        Err(err) => return Err(CmdError::spawn(command, err)),
    };
    if !output.status.success() {
        return Err(CmdError::exit_code(command, output));
    }
    Ok((command, output))
}

#[derive(Debug)]
pub(crate) struct CmdError {
    inner: Box<CmdErrorInner>, // `CmdErrorInner` is quite large
}

#[derive(Debug)]
struct CmdErrorInner {
    command: Command,
    kind: CmdErrorKind,
}

#[derive(Debug)]
enum CmdErrorKind {
    Spawn(std::io::Error),
    ExitCode(Output),
    Utf8(Output, Utf8Error),
    Json(Output, serde_json::Error),
}

impl CmdError {
    fn new(command: Command, kind: CmdErrorKind) -> Self {
        let inner = Box::new(CmdErrorInner { command, kind });
        Self { inner }
    }

    fn spawn(command: Command, io_error: std::io::Error) -> Self {
        let kind = CmdErrorKind::Spawn(io_error);
        Self::new(command, kind)
    }

    fn exit_code(command: Command, output: Output) -> Self {
        let kind = CmdErrorKind::ExitCode(output);
        Self::new(command, kind)
    }

    fn utf8(command: Command, output: Output, utf8_error: Utf8Error) -> Self {
        let kind = CmdErrorKind::Utf8(output, utf8_error);
        Self::new(command, kind)
    }

    fn json(command: Command, output: Output, json_error: serde_json::Error) -> Self {
        let kind = CmdErrorKind::Json(output, json_error);
        Self::new(command, kind)
    }

    fn output(&self) -> Option<&Output> {
        match &self.inner.kind {
            CmdErrorKind::Spawn(_) => None,
            CmdErrorKind::ExitCode(output) => Some(output),
            CmdErrorKind::Utf8(output, _) => Some(output),
            CmdErrorKind::Json(output, _) => Some(output),
        }
    }

    pub(crate) fn into_eyre(self) -> eyre::Report {
        let command_section = display_command(&self.inner.command)
            .to_string()
            .header("Command:");

        let stdout = self
            .output()
            .map(|output| &output.stdout[..])
            .unwrap_or(&[]);
        let stdout_section = String::from_utf8_lossy(stdout)
            .into_owned()
            .header("Captured stdout:");

        let stderr = self
            .output()
            .map(|output| &output.stderr[..])
            .unwrap_or(&[]);
        let stderr_section = String::from_utf8_lossy(stderr)
            .into_owned()
            .header("Captured stderr:");

        eyre::Report::from(self)
            .section(command_section)
            .section(stdout_section)
            .section(stderr_section)
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.kind {
            CmdErrorKind::Spawn(_) => write!(
                f,
                "failed to execute external program {}",
                display_program(&self.inner.command),
            ),
            CmdErrorKind::ExitCode(output) => write!(
                f,
                "external program {} did not finish successfully ({})",
                display_program(&self.inner.command),
                output.status,
            ),
            CmdErrorKind::Utf8(_, _) => write!(
                f,
                "output of external program {} is not valid utf-8",
                display_program(&self.inner.command),
            ),
            CmdErrorKind::Json(_, _) => write!(
                f,
                "failed to decode output of external program {}",
                display_program(&self.inner.command),
            ),
        }
    }
}

impl std::error::Error for CmdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner.kind {
            CmdErrorKind::Spawn(error) => Some(error),
            CmdErrorKind::ExitCode(_) => None,
            CmdErrorKind::Utf8(_, error) => Some(error),
            CmdErrorKind::Json(_, error) => Some(error),
        }
    }
}

fn display_command(command: &Command) -> impl fmt::Display {
    display::display_command_args(|| {
        [command.get_program()]
            .into_iter()
            .chain(command.get_args())
            .map(|s| s.to_string_lossy())
    })
}

fn display_program(command: &Command) -> impl fmt::Display {
    display::display_command_args(|| std::iter::once(command.get_program().to_string_lossy()))
}

fn display_args(command: &Command) -> impl fmt::Display {
    display::display_command_args(|| command.get_args().map(|arg| arg.to_string_lossy()))
}
