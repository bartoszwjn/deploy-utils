//! `status` subcommand.

use std::{fmt, process::Command};

use color_eyre::{Section, SectionExt};
use eyre::WrapErr;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    command::{self, CmdChild},
    display,
    profile::{ProfileInfo, Profiles},
};

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
    #[arg(long, short = 'j', default_value_t = 0)]
    eval_jobs: isize,

    /// Include profile store paths in the output.
    #[arg(long, short = 's')]
    show_paths: bool,
}

impl StatusArgs {
    pub(super) fn exec(self) -> eyre::Result<()> {
        let profiles = Profiles::eval(&self.flake)?.select(self.nodes.as_deref())?;
        anstream::println!("{}", profiles.display());

        let with_remote = self.query_deployed_profiles(&profiles)?;
        let results = self.eval_local_profiles(with_remote)?;

        anstream::println!("{}", self.display_results(&results));

        Ok(())
    }

    fn query_deployed_profiles<'a>(
        &self,
        profiles: &'a Profiles,
    ) -> eyre::Result<Vec<Vec<(&'a ProfileInfo, QueryResult)>>> {
        tracing::info!("querying deployed profile paths");

        // The main bottleneck here is network latency.
        // Start all connections immediately before waiting for

        let mut jobs = Vec::with_capacity(profiles.nodes().len());
        for node in profiles.nodes() {
            let mut node_jobs = Vec::with_capacity(node.profiles().len());
            for profile in node.profiles() {
                let span = tracing::error_span!(
                    "query_deployed_profile",
                    node = profile.node,
                    profile = profile.profile,
                );
                let job = span.in_scope(|| Self::spawn_query_job(profile))?;
                node_jobs.push((profile, span, job));
            }
            jobs.push(node_jobs);
        }

        let mut results = Vec::with_capacity(jobs.len());
        for node in jobs {
            let mut node_results = Vec::with_capacity(node.len());
            for (profile, span, job) in node.into_iter() {
                let result = span.in_scope(|| Self::resolve_query_job(job))?;
                node_results.push((profile, result));
            }
            results.push(node_results);
        }

        Ok(results)
    }

    fn spawn_query_job(profile: &ProfileInfo) -> eyre::Result<CmdChild> {
        const CHECK_PROFILE_SCRIPT: &str = "\
            if [ -L \"$1\" ]; then \
                deployed=$(realpath \"$1\"); \
                if [ \"$1\" = /nix/var/nix/profiles/system ]; then \
                    inner=$(dirname \"$(realpath \"$deployed/activate\")\"); \
                    active=$(realpath /run/current-system); \
                    if [ \"$inner\" = \"$active\" ]; then \
                        printf \"valid;%s\" \"$deployed\"; \
                    else \
                        printf \"needs reboot;%s\" \"$deployed\"; \
                    fi \
                else \
                    printf \"valid;%s\" \"$deployed\"; \
                fi \
            elif [ -e \"$1\" ]; then \
                printf \"invalid;\"; \
            else \
                printf \"missing;\"; \
            fi\
        ";

        // ssh runs the given command by concatenating arguments with spaces
        // and running that in a shell,
        // so we need to take care of quoting ourselves.
        let quote = |arg: &str| format!("'{}'", arg.replace('\'', "'\\''"));

        let mut cmd = Command::new("ssh");
        cmd.args(["-T", "-o", "ConnectTimeout=3"]);
        cmd.args(&profile.ssh_opts);
        cmd.args(["-o", &format!("User={}", profile.ssh_user)]);
        cmd.args([
            &profile.hostname,
            "--",
            "/bin/sh",
            "-c",
            &quote(CHECK_PROFILE_SCRIPT),
            "sh",
            &quote(&profile.profile_path),
        ]);

        command::spawn_piped(cmd).map_err(|err| err.into_eyre())
    }

    fn resolve_query_job(job: CmdChild) -> eyre::Result<QueryResult> {
        let output = match job.wait_with_output() {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(output.stderr());
                if !stderr.is_empty() {
                    tracing::warn!(
                        "ssh emitted warnings:\n  Captured stderr:\n{}",
                        display::indent(4, &stderr),
                    );
                }
                output.string().map_err(|error| error.into_eyre())?
            }
            Err(error) if error.is_exit_code_error() => {
                let stderr = String::from_utf8_lossy(error.stderr().unwrap_or(&[]));
                if stderr.is_empty() {
                    tracing::error!("ssh failed:\n  Captured stderr is empty");
                } else {
                    tracing::error!(
                        "ssh failed:\n  Captured stderr:\n{}",
                        display::indent(4, &stderr),
                    );
                }
                return Ok(QueryResult::Unknown);
            }
            Err(error) => return Err(error.into_eyre()),
        };

        QueryResult::parse(&output).ok_or_else(|| {
            if output.is_empty() {
                eyre::eyre!("external program ssh did not produce any output")
            } else {
                eyre::eyre!("external program ssh produced unexpected output")
                    .section(output.header("Captured stdout:"))
            }
        })
    }

    fn eval_local_profiles<'a>(
        &self,
        profiles: Vec<Vec<(&'a ProfileInfo, QueryResult)>>,
    ) -> eyre::Result<Vec<Vec<EvalResult<'a>>>> {
        tracing::info!("evaluating local profile paths");

        self.build_thread_pool()?.install(|| {
            profiles
                .into_par_iter()
                .map(|node| {
                    node.into_par_iter()
                        .map(|(profile, query_result)| {
                            let local_path = self.eval_local_profile(profile, &query_result)?;
                            Ok((profile, query_result, local_path))
                        })
                        .collect()
                })
                .collect()
        })
    }

    fn build_thread_pool(&self) -> eyre::Result<rayon::ThreadPool> {
        let num_threads: usize = match self.eval_jobs {
            positive @ 1_isize.. => positive as usize,
            below_zero @ ..=0 => {
                let available = match std::thread::available_parallelism() {
                    Ok(non_zero) => non_zero.get(),
                    Err(error) => {
                        tracing::warn!(
                            error = &error as &(dyn std::error::Error + Send + Sync),
                            "failed to query the number of available threads, using 1 thread",
                        );
                        1
                    }
                };
                available.saturating_add_signed(below_zero).max(1)
            }
        };

        assert!(0 < num_threads);
        tracing::debug!(num_threads, "starting thread pool");
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .wrap_err("thread pool creation failed")
    }

    #[tracing::instrument(
        level = "error",
        skip_all,
        fields(node = profile.node, profile = profile.profile),
    )]
    fn eval_local_profile(
        &self,
        profile: &ProfileInfo,
        query_result: &QueryResult,
    ) -> eyre::Result<Option<String>> {
        if !(matches!(query_result, QueryResult::Valid { .. }) || self.show_paths) {
            tracing::debug!("skipping local path evaluation");
            return Ok(None);
        }

        let mut cmd = Command::new("nix");
        cmd.args(["eval", "--json", "--"]);
        cmd.arg(format!(
            // TODO: find a way to properly escape node and profile name
            "{}#.deploy.nodes.{}.profiles.{}.path.outPath",
            self.flake, profile.node, profile.profile
        ));

        match command::output(cmd).and_then(|o| o.json::<String>()) {
            Ok(local_path) => Ok(Some(local_path)),
            Err(error) if error.is_exit_code_error() => {
                let stderr = String::from_utf8_lossy(error.stderr().unwrap_or(&[]));
                if stderr.is_empty() {
                    tracing::error!("nix eval failed:\n  Captured stderr is empty");
                } else {
                    tracing::error!(
                        "nix eval failed:\n  Catpured stderr:\n{}",
                        display::indent(4, &stderr),
                    );
                }
                Ok(None)
            }
            Err(error) if error.is_json_error() => {
                tracing::error!(error = &error as &(dyn std::error::Error + Send + Sync));
                Ok(None)
            }
            Err(error) => Err(error.into_eyre()),
        }
    }

    fn display_results(&self, results: &[Vec<EvalResult<'_>>]) -> impl fmt::Display {
        use display::styles::{HEADER, NODE, PROFILE};

        let node_width = display::get_max_width(
            results
                .iter()
                .flat_map(|node| node.first())
                .map(|(profile, _, _)| &profile.node),
        );
        let profile_width = display::get_max_width(
            results
                .iter()
                .flat_map(|node| node.iter())
                .map(|(profile, _, _)| &profile.profile),
        );

        fmt::from_fn(move |f| {
            writeln!(f, "{HEADER}Status:{HEADER:#}")?;
            for node in results {
                let mut first = true;
                for (profile, query_result, local_path) in node {
                    if first {
                        write!(f, "  {NODE}{:node_width$}{NODE:#}", profile.node)?;
                        first = false;
                    } else {
                        write!(f, "  {:node_width$}", "")?;
                    }

                    let status = ProfileStatus::from_paths(query_result, local_path.as_deref());
                    writeln!(
                        f,
                        " {PROFILE}{:profile_width$}{PROFILE:#} {}",
                        profile.profile,
                        status.display(node_width, self.show_paths),
                    )?;
                }
            }
            Ok(())
        })
    }
}

type EvalResult<'a> = (&'a ProfileInfo, QueryResult, Option<String>);

#[derive(Debug)]
enum QueryResult {
    Valid {
        deployed_path: String,
        needs_reboot: bool,
    },
    Invalid,
    Missing,
    Unknown,
}

impl QueryResult {
    fn parse(s: &str) -> Option<Self> {
        if let Some(deployed_path) = s.strip_prefix("valid;") {
            Some(Self::Valid {
                deployed_path: deployed_path.to_owned(),
                needs_reboot: false,
            })
        } else if let Some(deployed_path) = s.strip_prefix("needs reboot;") {
            Some(Self::Valid {
                deployed_path: deployed_path.to_owned(),
                needs_reboot: true,
            })
        } else if s == "invalid;" {
            Some(Self::Invalid)
        } else if s == "missing;" {
            Some(Self::Missing)
        } else {
            None
        }
    }
}

#[derive(Debug)]
enum ProfileStatus<'a> {
    UpToDate {
        path: &'a str,
    },
    NeedsReboot {
        path: &'a str,
    },
    Outdated {
        deployed_path: &'a str,
        local_path: &'a str,
    },
    Invalid {
        local_path: Option<&'a str>,
    },
    Missing {
        local_path: Option<&'a str>,
    },
    Unknown {
        deployed_path: Option<&'a str>,
        local_path: Option<&'a str>,
    },
}

impl<'a> ProfileStatus<'a> {
    fn from_paths(remote: &'a QueryResult, local_path: Option<&'a str>) -> Self {
        match remote {
            QueryResult::Valid {
                deployed_path,
                needs_reboot,
            } => match local_path {
                Some(local_path) if local_path == deployed_path => {
                    if *needs_reboot {
                        Self::NeedsReboot { path: local_path }
                    } else {
                        Self::UpToDate { path: local_path }
                    }
                }
                Some(local_path) => Self::Outdated {
                    deployed_path,
                    local_path,
                },
                None => Self::Unknown {
                    deployed_path: Some(deployed_path),
                    local_path: None,
                },
            },
            QueryResult::Invalid => Self::Invalid { local_path },
            QueryResult::Missing => Self::Missing { local_path },
            QueryResult::Unknown => Self::Unknown {
                deployed_path: None,
                local_path,
            },
        }
    }

    fn display(&self, node_width: usize, show_paths: bool) -> impl fmt::Display {
        fmt::from_fn(move |f| {
            write!(f, "{}", self.display_summary())?;
            if show_paths {
                match self {
                    Self::UpToDate { path } | Self::NeedsReboot { path } => {
                        write!(f, " {path}")?;
                    }
                    Self::Outdated {
                        deployed_path,
                        local_path,
                    } => {
                        write!(f, "\n  {:node_width$}   deployed path: {deployed_path}", "")?;
                        write!(f, "\n  {:node_width$}   local path:    {local_path}", "")?;
                    }
                    Self::Invalid { local_path } | Self::Missing { local_path } => {
                        if let Some(local_path) = local_path {
                            write!(f, "\n  {:node_width$}   local path: {local_path}", "")?;
                        }
                    }
                    Self::Unknown {
                        deployed_path,
                        local_path,
                    } => {
                        if let Some(deployed_path) = deployed_path {
                            write!(f, "\n  {:node_width$}   deployed path: {deployed_path}", "")?;
                        }
                        if let Some(local_path) = local_path {
                            write!(f, "\n  {:node_width$}   local path:    {local_path}", "")?;
                        }
                    }
                }
            }
            Ok(())
        })
    }

    fn display_summary(&self) -> impl fmt::Display {
        use display::styles::{FAILURE, SUCCESS, UNKNOWN, WARNING};

        fmt::from_fn(move |f| match self {
            ProfileStatus::UpToDate { .. } => write!(f, "{SUCCESS}up to date{SUCCESS:#}"),
            ProfileStatus::NeedsReboot { .. } => write!(f, "{WARNING}needs reboot{WARNING:#}"),
            ProfileStatus::Outdated { .. } => write!(f, "{WARNING}outdated{WARNING:#}"),
            ProfileStatus::Invalid { .. } => write!(f, "{FAILURE}invalid{FAILURE:#}"),
            ProfileStatus::Missing { .. } => write!(f, "{WARNING}missing{WARNING:#}"),
            ProfileStatus::Unknown { .. } => write!(f, "{UNKNOWN}unknown{UNKNOWN:#}"),
        })
    }
}
