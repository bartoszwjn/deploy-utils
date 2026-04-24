//! Querying information about `deploy-rs` profiles.

use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    process::Command,
};

use color_eyre::Section;
use eyre::WrapErr;

use crate::{command, display, natural_sort::NaturalString};

#[derive(Debug)]
pub(crate) struct Profiles {
    nodes: BTreeMap<NaturalString<'static>, NodeInfo>,
}

#[derive(Debug)]
pub(crate) struct NodeInfo {
    profiles: BTreeMap<NaturalString<'static>, ProfileInfo>,
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
            nodes.insert(NaturalString::owned(node_name), NodeInfo { profiles });
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
        self.nodes.values().map(|node| node.profiles.len()).sum()
    }

    pub(crate) fn display(&self) -> impl fmt::Display {
        use display::styles::{HEADER, NODE};

        let node_width = display::get_max_width(self.nodes.keys().map(|n| n.as_str()));
        let profile_width = display::get_max_width(self.profiles().map(|p| &p.profile));

        fmt::from_fn(move |f| {
            writeln!(f, "{HEADER}Profiles:{HEADER:#}")?;
            for (node, node_info) in &self.nodes {
                let mut first = true;
                for profile in node_info.profiles.values() {
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

    pub(crate) fn nodes(&self) -> impl ExactSizeIterator<Item = &NodeInfo> {
        self.nodes.values()
    }

    pub(crate) fn profiles(&self) -> impl Iterator<Item = &ProfileInfo> {
        self.nodes().flat_map(NodeInfo::profiles)
    }
}

impl NodeInfo {
    pub(crate) fn profiles(&self) -> impl ExactSizeIterator<Item = &ProfileInfo> {
        self.profiles.values()
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
        use display::styles::{FAST, HOSTNAME, PATH, PROFILE, SSH_OPTS, SSH_USER, USER};

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

    pub(crate) fn get_nix_sshopts(&self) -> String {
        // Nix 2.26 and later parse NIX_SSHOPTS as POSIX shell arguments:
        // https://github.com/NixOS/nix/pull/12020
        //
        // Earlier Nix versions and Lix (as of 2.95.1,
        // see https://git.lix.systems/lix-project/lix/src/tag/2.95.1/lix/libstore/ssh.cc#L39)
        // just split the value on whitespace.
        //
        // To try to make it work for both implementations,
        // we only quote values that contain characters that have special meaning for Nix 2.26+.
        // That way simple values will be parsed correctly by both implementations.
        // Complex values will be parsed correctly only by Nix 2.26+.

        fn needs_quoting(c: char) -> bool {
            match c {
                '\'' | '"' | '\\' => true,
                _ if c.is_whitespace() => true,
                _ => false,
            }
        }

        fn quote_arg(arg: &str) -> impl fmt::Display {
            fmt::from_fn(move |f| {
                if arg.is_empty() || arg.chars().any(needs_quoting) {
                    f.write_str("'")?;
                    let mut s = arg;
                    while !s.is_empty() {
                        match s.split_once('\'') {
                            Some((chunk, rest)) => {
                                f.write_str(chunk)?;
                                f.write_str("'\\''")?;
                                s = rest;
                            }
                            None => {
                                f.write_str(s)?;
                                s = "";
                            }
                        }
                    }
                    f.write_str("'")?;
                    Ok(())
                } else {
                    write!(f, "{}", arg)
                }
            })
        }

        let mut result = String::with_capacity(
            self.ssh_opts.iter().map(|opt| opt.len()).sum::<usize>() + self.ssh_opts.len(),
        );
        let mut first = true;
        for arg in &self.ssh_opts {
            let arg = quote_arg(arg);
            if first {
                write!(&mut result, "{}", arg).expect("");
                first = false;
            } else {
                write!(&mut result, " {}", arg).expect("");
            }
        }
        result
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

    let mut cmd = Command::new("nix");
    cmd.args([
        "eval",
        "--json",
        "--apply",
        REMOVE_PROFILE_PATH_EXPR,
        "--",
        &format!("{flake}#.deploy"),
    ]);
    command::output_json(cmd).map_err(|e| e.into_eyre())
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
