{
  pkgs,
  inputs,
  stateVersion,
  ...
}: let
  username = "noverby";
  homeDirectory = "/home/${username}";
in {
  home-manager = {
    useGlobalPkgs = false;
    useUserPackages = true;
    users.noverby = inputs.self.homeModules.noverby;
    extraSpecialArgs = {
      inherit inputs pkgs username homeDirectory stateVersion;
    };
  };
}
