{ node, profile }:
deploy:
let
  inherit (deploy.nodes.${node}.profiles.${profile}.path) drvPath outPath;
in
{
  drv = drvPath;
  out = outPath;
}
