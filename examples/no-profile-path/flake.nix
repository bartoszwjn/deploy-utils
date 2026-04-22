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
        nodes = {
          default = {
            hostname = "default.host";
            profiles = {
              default = {
                path = drv "default-default";
                user = "default-user";
              };
            };
          };
        };
      };
    };
}
