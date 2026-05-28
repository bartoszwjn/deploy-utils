//! `push` subcommand.

use std::{fmt, process::Command};

use crate::{
    command, display, nix,
    profile::{ProfileInfo, Profiles},
    target::Target,
};

use super::ProfileOptionOverrides;

/// `nix copy` all profile closures to their respective nodes without deploying them.
#[derive(clap::Args, Debug)]
pub(super) struct PushArgs {
    /// Push profiles to given target(s) only.
    ///
    /// The default is to push all profiles.
    ///
    /// A target can be a node name or a node and profile name separated by a dot.
    ///
    /// Node and profile names containing dots need to be surrounded with double quotes (`"`).
    #[arg(value_name = "TARGET")]
    targets: Option<Vec<Target>>,

    /// The flake to use as a source of profiles.
    #[arg(long, default_value = ".")]
    flake: String,

    #[command(flatten)]
    overrides: ProfileOptionOverrides,
}

impl PushArgs {
    pub(super) fn exec(self) -> eyre::Result<()> {
        let profiles =
            Profiles::eval(&self.flake, &self.overrides, false)?.select(self.targets.as_deref())?;
        anstream::println!("{}", profiles.display());

        let nodes = profiles.nodes();
        let mut results = Vec::with_capacity(nodes.len());
        for node in nodes {
            let profiles = node.profiles();
            let mut node_results = Vec::with_capacity(profiles.len());
            for profile in profiles {
                let status = self.push_profile(profile)?;
                node_results.push((profile, status));
            }
            results.push(node_results);
        }

        anstream::println!("{}", self.display_results(&results));

        Ok(())
    }

    #[tracing::instrument(
        level = "error",
        skip_all,
        fields(node = profile.node, profile = profile.profile),
    )]
    fn push_profile(&self, profile: &ProfileInfo) -> eyre::Result<Status> {
        let Some(eval_result) = self.eval_profile(profile)? else {
            return Ok(Status::Failure);
        };
        if let Status::Failure = self.build_profile(&eval_result.drv)? {
            return Ok(Status::Failure);
        }
        self.copy_profile(profile, &eval_result.out)
    }

    fn eval_profile(&self, profile: &ProfileInfo) -> eyre::Result<Option<EvalResult>> {
        tracing::info!("evaluating profile path");

        let mut cmd = Command::new("nix");
        cmd.args(["eval", "--json"])
            .args([
                "--apply",
                &format!(
                    "({}) {{ node = {}; profile = {}; }}",
                    include_str!("push/eval_profile.nix"),
                    nix::to_string_literal(&profile.node),
                    nix::to_string_literal(&profile.profile),
                ),
            ])
            .args(["--", &format!("{}#.deploy", self.flake)]);
        nix::run_eval::<EvalResult>(cmd)
    }

    fn build_profile(&self, drv_path: &str) -> eyre::Result<Status> {
        tracing::info!("building profile path");

        let mut cmd = Command::new("nix");
        cmd.args(["build", "--no-link", &format!("{drv_path}^*")]);

        match command::run(cmd) {
            Ok(()) => Ok(Status::Success),
            Err(error) if error.is_exit_code_error() => Ok(Status::Failure),
            Err(error) => Err(error.into_eyre()),
        }
    }

    fn copy_profile(&self, profile: &ProfileInfo, out_path: &str) -> eyre::Result<Status> {
        tracing::info!("copying profile closure");

        let mut cmd = Command::new("nix");
        cmd.arg("copy");
        if !profile.fast_connection {
            cmd.arg("--substitute-on-destination");
        }
        cmd.args([
            "--to",
            &format!("ssh://{}@{}", profile.ssh_user, profile.hostname),
            "--",
            out_path,
        ]);
        cmd.env("NIX_SSHOPTS", profile.get_nix_sshopts());

        match command::run(cmd) {
            Ok(()) => Ok(Status::Success),
            Err(error) if error.is_exit_code_error() => Ok(Status::Failure),
            Err(error) => Err(error.into_eyre()),
        }
    }

    fn display_results(&self, results: &[Vec<(&ProfileInfo, Status)>]) -> impl fmt::Display {
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

#[derive(Debug, serde::Deserialize)]
struct EvalResult {
    drv: String,
    out: String,
}

#[derive(Debug)]
enum Status {
    Success,
    Failure,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use display::styles::{FAILURE, SUCCESS};

        match self {
            Self::Success => write!(f, "{SUCCESS}success{SUCCESS:#}"),
            Self::Failure => write!(f, "{FAILURE}failure{FAILURE:#}"),
        }
    }
}
