//! Querying information about `deploy-rs` profiles.

use std::collections::BTreeMap;

use color_eyre::Section;
use eyre::WrapErr;

use crate::command;

#[allow(dead_code)] // TODO: remove
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

impl ProfileInfo {
    pub(crate) fn query(flake: &str, nodes: Option<&[String]>) -> eyre::Result<Vec<Self>> {
        tracing::debug!(flake, "evaluating deploy profiles");

        let mut deploy = eval_deploy(flake)?;

        let mut profiles = Vec::new();
        // `std::mem::take` the collections before iterating so that `deploy` and `node`
        // remain fully initialized and can be passed by reference to `Self::make`.
        for (node_name, mut node) in std::mem::take(&mut deploy.nodes) {
            for (profile_name, profile) in std::mem::take(&mut node.profiles) {
                let profile_info = Self::make(&node_name, profile_name, &deploy, &node, profile)?;
                profiles.push(profile_info);
            }
        }

        match nodes {
            None => Ok(profiles),
            Some(nodes) => {
                for node in nodes {
                    let has_profiles = profiles.iter().any(|p| &p.node == node);
                    if !has_profiles {
                        let mut all_nodes: Vec<_> =
                            profiles.iter().map(|p| p.node.as_str()).collect();
                        all_nodes.sort();
                        all_nodes.dedup();

                        return Err(eyre::eyre!("no profiles defined for node {node}")).note(
                            format!(
                                "profiles exist for the following nodes: {}",
                                all_nodes.join(", ")
                            ),
                        );
                    }
                }

                let filtered = profiles
                    .into_iter()
                    .filter(|p| nodes.iter().any(|n| n == &p.node))
                    .collect();
                Ok(filtered)
            }
        }
    }

    fn make(
        node_name: &str,
        profile_name: String,
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
            match (user.as_str(), profile_name.as_str()) {
                ("root", "system") => "/nix/var/nix/profiles/system".to_owned(),
                ("root", _) => {
                    format!("/nix/var/nix/profiles/per-user/root/{profile_name}")
                }
                (_, _) => {
                    let n = node_name;
                    let p = profile_name.as_str();
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
            profile: profile_name,
            hostname: node.hostname.clone(),
            profile_path,
            ssh_user,
            ssh_opts,
            user,
            fast_connection,
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
