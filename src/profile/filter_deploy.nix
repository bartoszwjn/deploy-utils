# The `path` attribute of a profile is the only part that can be expensive to evaluate,
# we can keep everything else.
deploy:
deploy
// {
  nodes = builtins.mapAttrs (
    name: node:
    node
    // {
      profiles = builtins.mapAttrs (name: profile: builtins.removeAttrs profile [ "path" ]) node.profiles;
    }
  ) deploy.nodes;
}
