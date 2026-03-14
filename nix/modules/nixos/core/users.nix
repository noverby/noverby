{
  pkgs,
  inputs,
  ...
}: let
  inherit (inputs.self.secrets) publicKeys;
in {
  environment.profiles = ["$HOME/.local"];
  users.users.noverby = {
    shell = pkgs.pkgsUnstable.nushell;
    isNormalUser = true;
    description = "Niclas Overby";
    extraGroups = ["networkmanager" "wheel" "docker" "libvirtd" "wireshark" "input" "kvm"];
    openssh.authorizedKeys.keys = [publicKeys.noverby-ssh-ed25519];
  };
}
