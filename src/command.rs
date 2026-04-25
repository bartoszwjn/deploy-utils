//! Running external commands.

use std::{
    fmt,
    process::{Child, Command, ExitStatus, Output, Stdio},
    str::Utf8Error,
};

use color_eyre::{Section, SectionExt};

use crate::display;

pub(crate) fn run(mut command: Command) -> Result<(), CmdError> {
    trace_program(&command);
    let status = match command.status() {
        Ok(status) => status,
        Err(err) => return Err(CmdError::spawn(Box::new(command), err)),
    };
    if !status.success() {
        return Err(CmdError::exit_code_status(Box::new(command), status));
    }
    Ok(())
}

pub(crate) fn output(command: Command) -> Result<CmdOutput, CmdError> {
    let mut command = Box::new(command);

    trace_program(&command);
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => return Err(CmdError::spawn(command, err)),
    };
    if !output.status.success() {
        return Err(CmdError::exit_code_output(command, Box::new(output)));
    }
    Ok(CmdOutput { output, command })
}

pub(crate) fn spawn_piped(command: Command) -> Result<CmdChild, CmdError> {
    let mut command = Box::new(command);

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    trace_program(&command);
    match command.spawn() {
        Ok(child) => Ok(CmdChild { child, command }),
        Err(error) => Err(CmdError::spawn(command, error)),
    }
}

#[derive(Debug)]
pub(crate) struct CmdOutput {
    output: Output,
    command: Box<Command>,
}

impl CmdOutput {
    pub(crate) fn string(mut self) -> Result<String, CmdError> {
        match String::from_utf8(self.output.stdout) {
            Ok(string) => Ok(string),
            Err(error) => {
                let utf8_error = error.utf8_error();
                self.output.stdout = error.into_bytes();
                Err(CmdError::utf8(
                    self.command,
                    Box::new(self.output),
                    utf8_error,
                ))
            }
        }
    }

    pub(crate) fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, CmdError> {
        match serde_json::from_slice(&self.output.stdout) {
            Ok(value) => Ok(value),
            Err(error) => Err(CmdError::json(self.command, Box::new(self.output), error)),
        }
    }

    pub(crate) fn stderr(&self) -> &[u8] {
        &self.output.stderr
    }
}

#[derive(Debug)]
pub(crate) struct CmdChild {
    child: Child,
    command: Box<Command>,
}

impl CmdChild {
    pub(crate) fn wait_with_output(self) -> Result<CmdOutput, CmdError> {
        trace_wait(&self.command);
        let output = match self.child.wait_with_output() {
            Ok(output) => output,
            Err(error) => return Err(CmdError::wait(self.command, error)),
        };
        if !output.status.success() {
            return Err(CmdError::exit_code_output(self.command, Box::new(output)));
        }

        Ok(CmdOutput {
            output,
            command: self.command,
        })
    }
}

#[derive(Debug)]
pub(crate) struct CmdError {
    command: Box<Command>,
    kind: CmdErrorKind,
}

#[derive(Debug)]
enum CmdErrorKind {
    Spawn(std::io::Error),
    Wait(std::io::Error),
    ExitCodeStatus(ExitStatus),
    ExitCodeOutput(Box<Output>),
    Utf8(Box<Output>, Utf8Error),
    Json(Box<Output>, serde_json::Error),
}

impl CmdError {
    fn spawn(command: Box<Command>, io_error: std::io::Error) -> Self {
        let kind = CmdErrorKind::Spawn(io_error);
        Self { command, kind }
    }

    fn wait(command: Box<Command>, io_error: std::io::Error) -> Self {
        let kind = CmdErrorKind::Wait(io_error);
        Self { command, kind }
    }

    fn exit_code_status(command: Box<Command>, status: ExitStatus) -> Self {
        let kind = CmdErrorKind::ExitCodeStatus(status);
        Self { command, kind }
    }

    fn exit_code_output(command: Box<Command>, output: Box<Output>) -> Self {
        let kind = CmdErrorKind::ExitCodeOutput(output);
        Self { command, kind }
    }

    fn utf8(command: Box<Command>, output: Box<Output>, utf8_error: Utf8Error) -> Self {
        let kind = CmdErrorKind::Utf8(output, utf8_error);
        Self { command, kind }
    }

    fn json(command: Box<Command>, output: Box<Output>, json_error: serde_json::Error) -> Self {
        let kind = CmdErrorKind::Json(output, json_error);
        Self { command, kind }
    }

    fn output(&self) -> Option<&Output> {
        match &self.kind {
            CmdErrorKind::Spawn(_) => None,
            CmdErrorKind::Wait(_) => None,
            CmdErrorKind::ExitCodeStatus(_) => None,
            CmdErrorKind::ExitCodeOutput(output) => Some(output),
            CmdErrorKind::Utf8(output, _) => Some(output),
            CmdErrorKind::Json(output, _) => Some(output),
        }
    }

    pub(crate) fn is_exit_code_error(&self) -> bool {
        match self.kind {
            CmdErrorKind::Spawn(_) => false,
            CmdErrorKind::Wait(_) => false,
            CmdErrorKind::ExitCodeStatus(_) => true,
            CmdErrorKind::ExitCodeOutput(_) => true,
            CmdErrorKind::Utf8(_, _) => false,
            CmdErrorKind::Json(_, _) => false,
        }
    }

    pub(crate) fn stderr(&self) -> Option<&[u8]> {
        self.output().map(|output| output.stderr.as_slice())
    }

    pub(crate) fn into_eyre(self) -> eyre::Report {
        let command_section = display_command(&self.command)
            .to_string()
            .header("Command:");

        let env_section = display_env(&self.command)
            .to_string()
            .header("Environment overrides:");

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
            .section(env_section)
            .section(stdout_section)
            .section(stderr_section)
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CmdErrorKind::Spawn(_) => write!(
                f,
                "failed to execute external program {}",
                display_program(&self.command),
            ),
            CmdErrorKind::Wait(_) => write!(
                f,
                "failed to wait for external program {} to finish",
                display_program(&self.command)
            ),
            CmdErrorKind::ExitCodeStatus(status) => write!(
                f,
                "external program {} did not finish successfully ({})",
                display_program(&self.command),
                status,
            ),
            CmdErrorKind::ExitCodeOutput(output) => write!(
                f,
                "external program {} did not finish successfully ({})",
                display_program(&self.command),
                output.status,
            ),
            CmdErrorKind::Utf8(_, _) => write!(
                f,
                "output of external program {} is not valid utf-8",
                display_program(&self.command),
            ),
            CmdErrorKind::Json(_, _) => write!(
                f,
                "failed to decode output of external program {}",
                display_program(&self.command),
            ),
        }
    }
}

impl std::error::Error for CmdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            CmdErrorKind::Spawn(error) => Some(error),
            CmdErrorKind::Wait(error) => Some(error),
            CmdErrorKind::ExitCodeStatus(_) => None,
            CmdErrorKind::ExitCodeOutput(_) => None,
            CmdErrorKind::Utf8(_, error) => Some(error),
            CmdErrorKind::Json(_, error) => Some(error),
        }
    }
}

fn trace_program(command: &Command) {
    tracing::trace!(
        program = %command.get_program().display(),
        args = %display_args(command),
        "executing external program",
    );
}

fn trace_wait(command: &Command) {
    tracing::trace!(
        program = %command.get_program().display(),
        args = %display_args(command),
        "waiting for external program to finish",
    );
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

fn display_env(command: &Command) -> impl fmt::Display {
    fmt::from_fn(move |f| {
        for (name, value) in command.get_envs() {
            match value {
                Some(value) => writeln!(f, "{}={}", name.display(), value.display())?,
                None => writeln!(f, "unset {}", name.display())?,
            }
        }
        Ok(())
    })
}
