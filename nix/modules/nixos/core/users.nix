{pkgs, ...}: {
  environment.profiles = ["$HOME/.local"];
  users.users.noverby = {
    shell = pkgs.nushell;
    isNormalUser = true;
    description = "Niclas Overby";
    extraGroups = ["networkmanager" "wheel" "docker" "libvirtd" "wireshark" "input"];
  };
}
