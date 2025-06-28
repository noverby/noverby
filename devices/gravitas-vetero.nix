{
  inputs,
  src,
  ...
}: {
  specialArgs = {
    inherit src inputs;
    stateVersion = "25.05";
  };
  modules = with inputs; [
    {
      nix.settings = {
        substituters = ["https://cosmic.cachix.org/"];
        trusted-public-keys = ["cosmic.cachix.org-1:Dya9IyXD4xdBehWjrkPv6rtxpmMdRel02smYzA85dPE="];
      };
    }
    nixos-cosmic.nixosModules.default
    home-manager.nixosModules.home-manager
    self.nixosModules.dell-xps-9320
    self.nixosModules.cosmic
    self.nixosModules.gnome
    self.nixosModules.configuration
    self.nixosModules.home-manager
  ];
}
