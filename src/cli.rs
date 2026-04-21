use clap::Parser;

/// Utilities for working with `deploy-rs`
#[derive(Debug, Parser)]
#[command(version)]
pub struct DeployUtilsApp {}

impl DeployUtilsApp {
    pub fn exec(self) -> eyre::Result<()> {
        todo!()
    }
}
