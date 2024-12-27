{
  pkgs,
  src,
  inputs,
  stateVersion,
  ...
}: let
  username = "noverby";
  homeDirectory = "/home/${username}";
in {
  home-manager = {
    useUserPackages = true;
    useGlobalPkgs = true;
    users.noverby = inputs.self.homeModules.noverby;
    extraSpecialArgs = {
      inherit inputs pkgs username homeDirectory stateVersion;
    };
  };
}
