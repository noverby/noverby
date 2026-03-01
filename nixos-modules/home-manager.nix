{
  pkgs,
  inputs,
  stateVersion,
  src,
  ...
}: {
  home-manager = {
    inherit (inputs.self) users;
    useGlobalPkgs = true;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs stateVersion src;
    };
  };
}
