{
  inputs,
  src,
  ...
}: {
  specialArgs = {
    inherit src inputs;
    stateVersion = "24.05";
  };
  modules = with inputs; [
    nixos-hardware
    .nixosModules
    .framework-13th-gen-intel
    {
      nix.settings = {
        substituters = ["https://cosmic.cachix.org/"];
        trusted-public-keys = ["cosmic.cachix.org-1:Dya9IyXD4xdBehWjrkPv6rtxpmMdRel02smYzA85dPE="];
      };
    }
    nixos-cosmic.nixosModules.default
    home-manager.nixosModules.home-manager
    self.nixosModules.framework
    self.nixosModules.gnome
    self.nixosModules.configuration
    self.nixosModules.home-manager
  ];
}
