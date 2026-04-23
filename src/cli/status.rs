//! `status` subcommand.

use crate::profile::Profiles;

/// Check if deployed profiles match local configuration.
#[derive(clap::Args, Debug)]
pub(super) struct StatusArgs {
    /// Check profiles from the given node(s) only.
    nodes: Option<Vec<String>>,

    /// The flake to use as a source of profiles.
    #[arg(long, default_value = ".")]
    flake: String,

    /// Number of Nix evaluations to perform in parallel.
    ///
    /// Zero means "as many as there are available threads",
    /// a negative number `-N` means "`N` fewer than the number of available threads".
    #[arg(long, default_value_t = 0)]
    eval_jobs: isize,

    /// Evaluate all store paths with a single invocation of Nix.
    #[arg(long, conflicts_with("eval_jobs"))]
    single_eval: bool,

    /// Include profile store paths in the output.
    #[arg(long)]
    show_paths: bool,
}

impl StatusArgs {
    pub(super) fn exec(self) -> eyre::Result<()> {
        let profiles = Profiles::eval(&self.flake)?.select(self.nodes.as_deref())?;
        dbg!(profiles);
        todo!("status")
    }
}
