use std::process::ExitCode;

use anstream::AutoStream;
use anstyle::{AnsiColor, Style};
use clap::Parser;
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use deploy_utils::DeployUtilsApp;

fn main() -> ExitCode {
    match exec() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            const RED_BOLD: Style = AnsiColor::Red.on_default().bold();
            anstream::eprintln!("{RED_BOLD}error:{RED_BOLD:#} {error:?}");
            ExitCode::FAILURE
        }
    }
}

fn exec() -> eyre::Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .display_location_section(false)
        .capture_span_trace_by_default(false)
        .install()?;

    let app = DeployUtilsApp::parse();

    init_tracing(app.default_log_level());

    app.exec()
}

fn init_tracing(default_level: tracing::Level) {
    let env_filter = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    let color_choice = AutoStream::choice(&std::io::stderr());

    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(move || AutoStream::new(std::io::stderr().lock(), color_choice));

    tracing_subscriber::registry()
        .with(fmt)
        .with(tracing_error::ErrorLayer::default())
        .with(env_filter)
        .init();
}
