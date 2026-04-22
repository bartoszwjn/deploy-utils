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
        user = "default-user";
        sshOpts = [
          "-p"
          "22"
        ];

        nodes = {
          nixos = {
            hostname = "nixos.host";
            user = "root";

            profiles = {
              system = {
                path = drv "nixos-system";
              };

              otherRoot = {
                path = drv "nixos-otherRoot";
              };

              otherNonRoot = {
                path = drv "nixos-otherNonRoot";
                profilePath = "/nixos/otherNonRoot";
                user = "other-user";
              };
            };
          };

          with-jump = {
            hostname = "with-jump.host";
            sshOpts = [
              "-J"
              "proxy.host"
            ];

            profiles = {
              default = {
                path = drv "with-jump-default";
                profilePath = "/with-jump/default";
              };

              otherUser = {
                path = drv "with-jump-otherUser";
                profilePath = "/with-jump/otherUser";
                sshUser = "other-user";
                sshOpts = [
                  "-i"
                  "/custom/ssh/key"
                ];
              };
            };
          };

          fast = {
            hostname = "fast.host";
            fastConnection = true;

            profiles = {
              default = {
                path = drv "fast-default";
                profilePath = "/fast/default";
              };
            };
          };
        };
      };
    };
}
