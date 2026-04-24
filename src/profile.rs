//! Querying information about `deploy-rs` profiles.

use std::{collections::BTreeMap, fmt};

use anstyle::{AnsiColor, Style};
use color_eyre::Section;
use eyre::WrapErr;
use unicode_width::UnicodeWidthStr;

use crate::command;
use crate::natural_sort::NaturalString;

#[derive(Debug)]
pub(crate) struct Profiles {
    nodes: BTreeMap<NaturalString<'static>, BTreeMap<NaturalString<'static>, ProfileInfo>>,
}

#[derive(Debug)]
pub(crate) struct ProfileInfo {
    pub(crate) node: String,
    pub(crate) profile: String,
    pub(crate) hostname: String,
    pub(crate) profile_path: String,
    pub(crate) user: String,
    pub(crate) ssh_user: String,
    pub(crate) ssh_opts: Vec<String>,
    pub(crate) fast_connection: bool,
}

impl Profiles {
    pub(crate) fn eval(flake: &str) -> eyre::Result<Self> {
        tracing::debug!(flake, "evaluating deploy profiles");

        let mut deploy = eval_deploy(flake)?;
        let num_nodes = deploy.nodes.len();

        let mut nodes = BTreeMap::new();
        // `std::mem::take` the collections before iterating so that `deploy` and `node`
        // remain fully initialized and can be passed by reference to `Self::make`.
        for (node_name, mut node) in std::mem::take(&mut deploy.nodes) {
            let mut profiles = BTreeMap::new();
            for (profile_name, profile) in std::mem::take(&mut node.profiles) {
                let profile_info =
                    ProfileInfo::make(&node_name, &profile_name, &deploy, &node, profile)?;
                profiles.insert(NaturalString::owned(profile_name), profile_info);
            }
            nodes.insert(NaturalString::owned(node_name), profiles);
        }

        let this = Self { nodes };

        tracing::debug!(
            num_nodes,
            num_profiles = this.num_profiles(),
            "done evaluating deploy profiles",
        );

        Ok(this)
    }

    pub(crate) fn select(mut self, nodes: Option<&[String]>) -> eyre::Result<Self> {
        let Some(nodes) = nodes else {
            return Ok(self);
        };

        for node in nodes {
            if !self.nodes.contains_key(&NaturalString::borrowed(node)) {
                let all_nodes: Vec<_> = self.nodes.keys().map(|n| n.as_str()).collect();

                return Err(eyre::eyre!("no profiles defined for node {node}")).note(format!(
                    "profiles exist for the following nodes: {}",
                    all_nodes.join(", ")
                ));
            }
        }

        self.nodes
            .retain(|node, _| nodes.iter().any(|n| node.as_str() == n));

        tracing::debug!(
            num_nodes = self.nodes.len(),
            num_profiles = self.num_profiles(),
            "selected a subset of deploy profiles",
        );

        Ok(self)
    }

    fn num_profiles(&self) -> usize {
        self.nodes.values().map(|ps| ps.len()).sum()
    }

    fn profiles(&self) -> impl Iterator<Item = &ProfileInfo> {
        self.nodes.values().flat_map(|profiles| profiles.values())
    }

    pub(crate) fn display(&self) -> impl fmt::Display {
        const HEADER: Style = Style::new().bold();
        const NODE: Style = AnsiColor::Blue.on_default();

        let node_width = self
            .nodes
            .keys()
            .map(|n| n.as_str().width())
            .max()
            .unwrap_or(0);
        let profile_width = self
            .profiles()
            .map(|p| p.profile.width())
            .max()
            .unwrap_or(0);

        fmt::from_fn(move |f| {
            writeln!(f, "{HEADER}Profiles:{HEADER:#}")?;
            for (node, profiles) in &self.nodes {
                let mut first = true;
                for profile in profiles.values() {
                    let profile = profile.display(profile_width);
                    if first {
                        let node = node.as_str();
                        writeln!(f, "  {NODE}{node:node_width$}{NODE:#} {profile}")?;
                        first = false;
                    } else {
                        writeln!(f, "  {:node_width$} {profile}", "")?;
                    }
                }
            }
            Ok(())
        })
    }
}

impl ProfileInfo {
    fn make(
        node_name: &str,
        profile_name: &str,
        deploy: &Deploy,
        node: &Node,
        profile: Profile,
    ) -> eyre::Result<Self> {
        let generic_opts =
            combine_generic_options(&deploy.generic, &node.generic, &profile.generic);

        let user = (generic_opts.user)
            .or_else(|| generic_opts.ssh_user.clone())
            .ok_or_else(|| {
                eyre::eyre!(
                    "neither `user` nor `sshUser` is set \
                    for profile {profile_name} of node {node_name}"
                )
            })?;
        let ssh_user = match generic_opts.ssh_user {
            Some(ssh_user) => ssh_user,
            None => whoami::username().wrap_err("could not determine current user's username")?,
        };
        let ssh_opts = generic_opts.ssh_opts;
        let fast_connection = generic_opts.fast_connection.unwrap_or(false);

        let profile_path = if let Some(explicit) = profile.profile_path {
            explicit
        } else {
            match (user.as_str(), profile_name) {
                ("root", "system") => "/nix/var/nix/profiles/system".to_owned(),
                ("root", _) => {
                    format!("/nix/var/nix/profiles/per-user/root/{profile_name}")
                }
                (_, _) => {
                    let n = node_name;
                    let p = profile_name;
                    return Err(eyre::eyre!(
                        "cannot determine profile path for a non-root user {user}"
                    )
                    .suggestion(format!(
                        "specify `deploy.nodes.{n}.profiles.{p}.profilePath` explicitly, \
                        instead of relying on the deploy-rs default, \
                        which is determined dynamically during profile activation"
                    )));
                }
            }
        };

        Ok(Self {
            node: node_name.to_owned(),
            profile: profile_name.to_owned(),
            hostname: node.hostname.clone(),
            profile_path,
            ssh_user,
            ssh_opts,
            user,
            fast_connection,
        })
    }

    fn display(&self, profile_width: usize) -> impl fmt::Display {
        const PROFILE: Style = AnsiColor::Cyan.on_default();
        const USER: Style = AnsiColor::Yellow.on_default();
        const SSH_USER: Style = AnsiColor::Yellow.on_default();
        const HOSTNAME: Style = AnsiColor::Green.on_default();
        const PATH: Style = AnsiColor::Blue.on_default();
        const FAST: Style = AnsiColor::Red.on_default();
        const SSH_OPTS: Style = AnsiColor::Cyan.on_default();

        fmt::from_fn(move |f| {
            write!(
                f,
                "{PROFILE}{:profile_width$}{PROFILE:#} as {USER}{}{USER:#} \
                    at {SSH_USER}{}{SSH_USER:#}@{HOSTNAME}{}{HOSTNAME:#}:{PATH}{}{PATH:#}",
                self.profile, self.user, self.ssh_user, self.hostname, self.profile_path,
            )?;
            if self.fast_connection {
                write!(f, " {FAST}(fast){FAST:#}")?;
            }
            if !self.ssh_opts.is_empty() {
                write!(f, " with {SSH_OPTS}{}{SSH_OPTS:#}", self.ssh_opts.join(" "))?;
            }
            Ok(())
        })
    }
}

fn eval_deploy(flake: &str) -> eyre::Result<Deploy> {
    // The `path` attribute of a profile is the only part that can be expensive to evaluate,
    // we can keep everything else.
    const REMOVE_PROFILE_PATH_EXPR: &str = "\
        deploy: deploy // { \
            nodes = builtins.mapAttrs (name: node: node // { \
                profiles = builtins.mapAttrs (name: profile: \
                    builtins.removeAttrs profile [\"path\"]\
                ) node.profiles; \
            }) deploy.nodes; \
        }\
    ";

    command::output_json(
        "nix",
        [
            "eval",
            "--json",
            "--apply",
            REMOVE_PROFILE_PATH_EXPR,
            "--",
            &format!("{flake}#.deploy"),
        ],
    )
    .map_err(|e| e.into_eyre())
}

fn combine_generic_options(
    deploy: &GenericOptions,
    node: &GenericOptions,
    profile: &GenericOptions,
) -> GenericOptions {
    let ssh_user = (profile.ssh_user.clone())
        .or_else(|| node.ssh_user.clone())
        .or_else(|| deploy.ssh_user.clone());

    let ssh_opts = (deploy.ssh_opts.iter())
        .chain(node.ssh_opts.iter())
        .chain(profile.ssh_opts.iter())
        .cloned()
        .collect();

    let user = (profile.user.clone())
        .or_else(|| node.user.clone())
        .or_else(|| deploy.user.clone());

    let fast_connection = (profile.fast_connection)
        .or(node.fast_connection)
        .or(deploy.fast_connection);

    GenericOptions {
        ssh_user,
        ssh_opts,
        user,
        fast_connection,
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Deploy {
    nodes: BTreeMap<String, Node>,

    #[serde(flatten)]
    generic: GenericOptions,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    hostname: String,
    profiles: BTreeMap<String, Profile>,

    #[serde(flatten)]
    generic: GenericOptions,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    profile_path: Option<String>,

    #[serde(flatten)]
    generic: GenericOptions,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericOptions {
    ssh_user: Option<String>,
    #[serde(default)]
    ssh_opts: Vec<String>,
    user: Option<String>,
    fast_connection: Option<bool>,
}
