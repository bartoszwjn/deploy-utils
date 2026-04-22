use copy::CopyArgs;
use status::StatusArgs;

mod copy;
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
#[command(next_help_heading = "Logging options")]
enum Subcommand {
    Copy(CopyArgs),
    Status(StatusArgs),
}

#[derive(clap::Args, Debug)]
struct LoggingOptions {
    /// Be less verbose.
    #[arg(long, short = 'q', action = clap::ArgAction::Count, global = true)]
    quiet: u8,

    /// Be more verbose.
    #[arg(long, short = 'v', action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

impl DeployUtilsApp {
    pub fn exec(self) -> eyre::Result<()> {
        match self.subcommand {
            Subcommand::Copy(copy_args) => copy_args.exec(),
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
