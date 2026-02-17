{
  devShells.nixos-rs = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };
  nixosConfigurations.nixos-rs = {
    inputs,
    lib,
    ...
  }: {
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
