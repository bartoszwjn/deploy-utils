# `deploy-utils`

A command-line program providing utilities for working with [`deploy-rs`].

## Status

Work-in-progress, breaking changes are expected.

## Installation

Use the Nix flake in this repository to install the package into a profile,
add it to a NixOS or Home Manager configuration,
or run it directly from the command line:

```bash
nix run github:bartoszwjn/deploy-utils -- status
```

`cargo install --git` should work as well.

The program expects `nix` and `ssh` commands to be available in `PATH` at runtime.

## Usage

See the `--help` output of each subcommand for details about all command line flags and options.

### `deploy-utils status`

Uses `ssh` to connect to selected nodes
and check the store paths deployed at each profile's `profilePath`,
then compares that to the profile's `path` evaluated from a given flake (`.` by default).

This provides an easy way to check which profiles need to be deployed
to bring everything in sync with the local configuration.

Local paths are evaluated by running multiple instances of `nix eval` in parallel,
which can make things significantly faster.

#### `needs reboot`

The program also tries to identify NixOS configurations deployed with `--boot`
that have not been restarted yet.

For profiles where `profilePath` is `/nix/var/nix/profiles/system`
(which is the path used for NixOS system configurations)
the query also compares the deployed profile with `/run/current-system`.
If `/run/current-system` isn't the same as the deployed profile,
the profile is reported as `needs reboot`.

### `deploy-utils push`

Uses `nix copy` to copy the store paths of selected profiles to their nodes without deploying them.

Currently this is fully sequential.

## Roadmap

Expected upcoming changes:

- use `sudo` when querying deployed profile path if `sshUser` and `user` are not the same
- parallelize the `push` subcommand
- use async to make parallelization less awkward
- use [`nix-eval-jobs`] for faster parallel evaluation and better control over memory usage

[`deploy-rs`]: https://github.com/serokell/deploy-rs
[`nix-eval-jobs`]: https://github.com/NixOS/nix-eval-jobs
