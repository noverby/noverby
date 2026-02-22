{
  pkgs,
  inputs,
  stateVersion,
  src,
  ...
}: {
  home-manager = {
    users = inputs.self.users;
    useGlobalPkgs = true;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs stateVersion src;
    };
  };
}
