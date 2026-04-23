//! `copy` subcommand.

use crate::profile::Profiles;

/// Copy profile closures to their respective nodes without deploying them.
#[derive(clap::Args, Debug)]
pub(super) struct CopyArgs {
    /// Copy profiles from the given node(s) only.
    nodes: Option<Vec<String>>,

    /// The flake to use as a source of profiles.
    #[arg(long, default_value = ".")]
    flake: String,

    /// Try substitutes on the destination node.
    ///
    /// The default is to follow each profile's `fastConnection` option.
    #[arg(long, short = 's')]
    substitute_on_destination: bool,

    /// Do not try substitutes on the destination node.
    ///
    /// The default is to follow each profile's `fastConnection` option.
    #[arg(long, short = 'S', conflicts_with = "substitute_on_destination")]
    no_substitute_on_destination: bool,
}

impl CopyArgs {
    pub(super) fn exec(self) -> eyre::Result<()> {
        let profiles = Profiles::eval(&self.flake)?.select(self.nodes.as_deref())?;
        dbg!(profiles);
        todo!("copy")
    }
}
