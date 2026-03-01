{
  pkgs,
  src,
  ...
}: let
  publicKeys = import "${src}/config/secrets/publicKeys.nix";
in {
  environment.profiles = ["$HOME/.local"];
  users.users.noverby = {
    shell = pkgs.nushell;
    isNormalUser = true;
    description = "Niclas Overby";
    extraGroups = ["networkmanager" "wheel" "docker" "libvirtd" "wireshark" "input" "kvm"];
    openssh.authorizedKeys.keys = [publicKeys.noverby-ssh-ed25519];
  };
}
