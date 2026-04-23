{
  outputs =
    inputs:
    let
      drv =
        name:
        derivation {
          inherit name;
          system = "x86_64-linux";
          builder = ../builder.sh;
        };
    in
    {
      deploy = {
        user = "root";
        sshUser = "root";
        sshOpts = [
          "-p"
          "22"
        ];

        nodes =
          let
            mkNode = name: area: {
              inherit name;
              value = {
                hostname = "${name}.${area}.subdomain.domain.tld";
                profiles.system.path = drv "${name}-system";
              };
            };

            mkArea =
              n: salt:
              "reg1-az"
              + builtins.substring 0 (if mod n 5 == 1 || mod n 7 == 4 then 2 else 1) (
                builtins.hashString "md5" "${toString n}-${salt}"
              );
            mod = n: m: n - (n / m) * m;

            euNodes = builtins.genList (n: mkNode "eu-srv-${toString (n + 1)}" (mkArea n "de")) 15;
            usNodes = builtins.genList (n: mkNode "us-srv-${toString (n + 1)}" (mkArea n "fi")) 6;
            customNodes = builtins.genList (n: mkNode "custom-node-${toString (n + 1)}" (mkArea n "custom")) 3;
          in
          builtins.listToAttrs (euNodes ++ usNodes ++ customNodes);
      };
    };
}
