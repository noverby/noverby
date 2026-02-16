{
  devShells.oxidized-nixos = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };
  nixosConfigurations.oxidized-nixos = {
    inputs,
    lib,
    ...
  }: {
    system = "x86_64-linux";
    modules = [
      ./base.nix
      ./systemd.nix
      ./bash.nix
      ./sudo.nix
      ./coreutils.nix
    ];
  };
}
