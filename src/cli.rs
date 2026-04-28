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
