{
  devShells.nixos-rs = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };
  nixosConfigurations.nixos-nix = _: {
    system = "x86_64-linux";
    modules = [
      ./base.nix
    ];
  };
  nixosConfigurations.nixos-rs = _: {
    system = "x86_64-linux";
    modules = [
      ./base.nix
      ./systemd.nix
      # ./bash.nix
      ./sudo.nix
      # ./coreutils.nix
    ];
  };
}
