{
  pkgs,
  inputs,
  stateVersion,
  src,
  hasSecrets ? true,
  ...
}: {
  home-manager = {
    inherit (inputs.self) users;
    useGlobalPkgs = true;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs stateVersion src;
      nixosConfig = {inherit hasSecrets;};
    };
  };
}
