//! Command-line interface.

use push::PushArgs;
use status::StatusArgs;

mod push;
mod status;

/// Utilities for working with `deploy-rs`
#[derive(clap::Parser, Debug)]
#[command(version)]
pub struct DeployUtilsApp {
    #[command(subcommand)]
    subcommand: Subcommand,

    #[command(flatten)]
    logging: LoggingOptions,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    Push(PushArgs),
    Status(StatusArgs),
}

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Profile option overrides")]
pub(crate) struct ProfileOptionOverrides {
    /// Override each profile's `hostname` with the given value.
    #[arg(long)]
    pub(crate) hostname: Option<String>,

    /// Override each profile's `user` with the given value.
    #[arg(long)]
    pub(crate) profile_user: Option<String>,

    /// Override each profile's `sshUser` with the given value.
    #[arg(long)]
    pub(crate) ssh_user: Option<String>,

    /// Override each profile's `sshOpts` with the given value(s).
    ///
    /// Note on parsing: after encountering `--ssh-opts` all further arguments will be treated
    /// as values for this option, until an optional end marker value `;` is encountered.
    ///
    /// Examples of mixing `--ssh-opts` with other options (all of these are equivalent):
    ///
    ///     # specify `--ssh-opts` last
    ///     <subcommand> --hostname foo --ssh-opts -p 22
    ///
    ///     # use the `;` end marker
    ///     <subcommand> --ssh-opts -p 22 ";" --hostname foo
    ///
    ///     # add one value at a time with `--ssh-opts=`
    ///     <subcommand> --ssh-opts=-p --ssh-opts=22 --hostname foo
    ///
    /// In most shells the `;` argument will need to be surrounded with quotes in order to avoid
    /// being interpreted by the shell as an end-of-command marker.
    #[arg(
        long,
        num_args = 0..,
        allow_hyphen_values = true,
        value_terminator = ";",
        verbatim_doc_comment,
    )]
    pub(crate) ssh_opts: Option<Vec<String>>,

    /// Override each profile's `fastConnection` with the given value.
    #[arg(long)]
    pub(crate) fast_connection: Option<bool>,
}

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Logging options")]
struct LoggingOptions {
    /// Be less verbose.
    ///
    /// Can be specified multiple times, with each instance further reducing log verbosity.
    ///
    /// Each instance of `--quiet` cancels out one instance of `--verbose` and vice versa.
    #[arg(long, short = 'q', action = clap::ArgAction::Count, global = true)]
    quiet: u8,

    /// Be more verbose.
    ///
    /// Can be specified multiple times, with each instance further increasing log verbosity.
    ///
    /// Each instance of `--verbose` cancels out one instance of `--quiet` and vice versa.
    #[arg(long, short = 'v', action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

impl DeployUtilsApp {
    pub fn exec(self) -> eyre::Result<()> {
        match self.subcommand {
            Subcommand::Push(push_args) => push_args.exec(),
            Subcommand::Status(status_args) => status_args.exec(),
        }
    }

    pub fn default_log_level(&self) -> tracing::Level {
        match i16::from(self.logging.verbose) - i16::from(self.logging.quiet) {
            ..=-2 => tracing::Level::ERROR,
            -1 => tracing::Level::WARN,
            0 => tracing::Level::INFO,
            1 => tracing::Level::DEBUG,
            2.. => tracing::Level::TRACE,
        }
    }
}
