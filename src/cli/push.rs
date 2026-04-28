//! `push` subcommand.

use std::{fmt, process::Command};

use crate::{
    command, display,
    profile::{ProfileInfo, Profiles},
};

/// `nix copy` all profile closures to their respective nodes without deploying them.
#[derive(clap::Args, Debug)]
pub(super) struct PushArgs {
    /// Push profiles to given node(s) only.
    ///
    /// The default is to push all profiles.
    /// When at least one node name is specified, only profiles from these nodes are pushed.
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

impl PushArgs {
    pub(super) fn exec(self) -> eyre::Result<()> {
        let profiles = Profiles::eval(&self.flake)?.select(self.nodes.as_deref())?;
        anstream::println!("{}", profiles.display());

        let nodes = profiles.nodes();
        let mut results = Vec::with_capacity(nodes.len());
        for node in nodes {
            let profiles = node.profiles();
            let mut node_results = Vec::with_capacity(profiles.len());
            for profile in profiles {
                let status = self.push_closure(profile)?;
                node_results.push((profile, status));
            }
            results.push(node_results);
        }

        anstream::println!("{}", self.display_results(&results));

        Ok(())
    }

    fn push_closure(&self, profile: &ProfileInfo) -> eyre::Result<PushStatus> {
        tracing::info!(
            node = profile.node,
            profile = profile.profile,
            "copying profile closure",
        );

        let substitute_on_destination = match (
            self.substitute_on_destination,
            self.no_substitute_on_destination,
            profile.fast_connection,
        ) {
            (true, false, _) => true,
            (false, true, _) => false,
            (true, true, _) => unreachable!("flags are mutually exclusive"),
            (false, false, is_fast) => !is_fast,
        };

        let mut cmd = Command::new("nix");
        cmd.arg("copy");
        if substitute_on_destination {
            cmd.arg("--substitute-on-destination");
        }
        cmd.args([
            "--to",
            &format!("ssh://{}@{}", profile.ssh_user, profile.hostname),
            "--",
            &format!(
                // TODO: find a way to properly escape node and profile name
                "{}#.deploy.nodes.{}.profiles.{}.path",
                self.flake, profile.node, profile.profile
            ),
        ]);
        cmd.env("NIX_SSHOPTS", profile.get_nix_sshopts());

        match command::run(cmd) {
            Ok(()) => Ok(PushStatus::Success),
            Err(error) if error.is_exit_code_error() => Ok(PushStatus::Failure),
            Err(error) => Err(error.into_eyre()),
        }
    }

    fn display_results(&self, results: &[Vec<(&ProfileInfo, PushStatus)>]) -> impl fmt::Display {
        use display::styles::{HEADER, NODE, PROFILE};

        let node_width = display::get_max_width(
            results
                .iter()
                .flat_map(|node| node.first())
                .map(|(profile, _)| &profile.node),
        );
        let profile_width = display::get_max_width(
            results
                .iter()
                .flat_map(|node| node.iter())
                .map(|(profile, _)| &profile.profile),
        );

        fmt::from_fn(move |f| {
            writeln!(f, "{HEADER}Push results:{HEADER:#}")?;
            for node in results {
                let mut first = true;
                for (profile, status) in node {
                    if first {
                        write!(f, "  {NODE}{:node_width$}{NODE:#}", profile.node)?;
                        first = false;
                    } else {
                        write!(f, "  {:node_width$}", "")?;
                    }
                    writeln!(
                        f,
                        " {PROFILE}{:profile_width$}{PROFILE:#} {}",
                        profile.profile, status,
                    )?;
                }
            }
            Ok(())
        })
    }
}

#[derive(Debug)]
enum PushStatus {
    Success,
    Failure,
}

impl fmt::Display for PushStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use display::styles::{FAILURE, SUCCESS};

        match self {
            Self::Success => write!(f, "{SUCCESS}success{SUCCESS:#}"),
            Self::Failure => write!(f, "{FAILURE}failure{FAILURE:#}"),
        }
    }
}
